//! Thief Book for OpenIDEA —— 摸鱼看小说插件。
//!
//! 底部工具窗口（web 视图 web/index.html）显示当前页；状态栏项显示书名与进度；
//! 全部状态落 storage，实例被 idle 回收或 IDE 重启都无损。
//! 书库 = 插件私有目录（plugin_dir）下的 *.txt。
//!
//! 插件 ↔ 页面协议（post_view_message / ViewEvent::Message）：
//! - 页面 → 插件：纯字符串 `hello` `next` `prev` `boss` `chapters` `switch`
//!   `font+ font- lines+ lines- spacing+ spacing-`
//! - 插件 → 页面：JSON `{"t":"page","boss":..,"font":..,"spacing":..,"status":..,"lines":[[段落文本,是否段末],..]}`
//!   或 `{"t":"msg","text":".."}`（空书库/错误提示）
//!
//! 分页按段落计：一"行" = txt 的一个自然段，长段由页面 CSS 自然折行（超页面高度时页面内滚动）。

mod novel;
mod view;

use novel::{Line, Novel};

use openidea_plugin::{host, register_plugin, Error, Invocation, NotificationLevel, PickItem, Plugin, StatusItem};
use openidea_plugin::views::{View, ViewEvent, Views};

const VIEW: &str = "reader";
const ITEM: &str = "thief-book";
const K_BOOK: &str = "book";
const K_HIDDEN: &str = "hidden";
const K_SETTINGS: &str = "settings";
const SAMPLE_NAME: &str = "示例小说.txt";

fn pos_key(book: &str) -> String {
    format!("pos:{book}")
}
fn pct_key(book: &str) -> String {
    format!("pct:{book}")
}

// ---------- storage 小工具 ----------

fn storage_get(key: &str) -> Option<String> {
    host::storage::get(key).ok().flatten()
}

fn storage_set(key: &str, value: &str) {
    if let Err(e) = host::storage::set(key, value) {
        host::log(openidea_plugin::LogLevel::Warn, &format!("storage 写入失败 {key}: {e:?}"));
    }
}

fn load_settings() -> view::Settings {
    storage_get(K_SETTINGS).and_then(|s| Some(view::Settings::decode(&s))).unwrap_or_default()
}

fn save_settings(s: &view::Settings) {
    storage_set(K_SETTINGS, &s.encode());
}

fn current_book() -> Option<String> {
    storage_get(K_BOOK).filter(|b| !b.is_empty())
}

fn hidden() -> bool {
    storage_get(K_HIDDEN).as_deref() == Some("1")
}

fn load_pos(book: &str) -> usize {
    storage_get(&pos_key(book)).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn save_progress(book: &str, pos: usize, pct: u32) {
    storage_set(&pos_key(book), &pos.to_string());
    storage_set(&pct_key(book), &pct.to_string());
}

// ---------- 书库 ----------

fn plugin_dir() -> Result<String, Error> {
    host::fs::plugin_dir()
}

fn join(dir: &str, name: &str) -> String {
    if dir.contains('\\') {
        format!(r"{dir}\{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// 书库清单：plugin_dir 下的 *.txt（不分大小写），按文件名排序。
fn library() -> Result<Vec<String>, Error> {
    let dir = plugin_dir()?;
    let mut books: Vec<String> = host::fs::read_dir(&dir)?
        .into_iter()
        .filter(|e| e.kind == openidea_plugin::FileKind::File && e.name.to_lowercase().ends_with(".txt"))
        .map(|e| e.name)
        .collect();
    books.sort();
    Ok(books)
}

fn book_path(book: &str) -> Result<String, Error> {
    Ok(join(&plugin_dir()?, book))
}

/// 读取 + 解码 + 解析（显示行 = 段落）。
fn open_book(book: &str, s: &view::Settings) -> Result<(Novel, Vec<Line>), String> {
    let path = book_path(book).map_err(|e| format!("拿不到书库路径：{e:?}"))?;
    let bytes = host::fs::read_bytes(&path).map_err(|e| format!("读不了《{book}》：{e:?}"))?;
    let (text, _enc) = novel::decode(&bytes)?;
    let n = Novel::parse(&text);
    let lines = n.build_lines();
    Ok((n, lines))
}

// ---------- JSON 小工具（页面协议） ----------

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn msg_json(text: &str) -> String {
    format!("{{\"t\":\"msg\",\"text\":{}}}", json_str(text))
}

// ---------- 动作 ----------

#[derive(Clone, Debug)]
enum Act {
    Next,
    Prev,
    Boss,
    NextCh,
    PrevCh,
    Jump,
    Chapters,
    Switch,
    Font(i32),
    Lines(i32),
    Spacing(i32),
    LibPath,
    Sample,
}

#[derive(Default)]
struct ThiefBook;

impl ThiefBook {
    /// 状态栏项：书名 + 进度。不读全书（用缓存的 pct）。
    fn status_item(&self) -> Option<StatusItem> {
        let book = current_book()?;
        let stem = book.trim_end_matches(".txt").to_owned();
        let pct = storage_get(&pct_key(&book)).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        Some(StatusItem {
            text: format!("📖 {stem} {pct}%"),
            tooltip: Some("Thief Book 阅读进度 · 点击跳页".into()),
            command: Some("jump".into()),
        })
    }

    /// 每次状态变更后统一刷 UI：状态栏 + 推送面板状态。
    fn sync_ui(&self) {
        if hidden() {
            host::remove_status_item(ITEM);
        } else if let Some(item) = self.status_item() {
            host::set_status_item(ITEM, &item);
        }
        self.push_state();
    }

    /// 把当前状态推给页面（web 视图）。
    fn push_state(&self) {
        let json = self.page_state();
        if let Err(e) = host::post_view_message(VIEW, &json) {
            host::log(openidea_plugin::LogLevel::Debug, &format!("推送面板状态失败（面板可能没开）：{e:?}"));
        }
    }

    /// 面板状态 JSON。页面据此渲染正文 / 老板屏 / 提示。
    fn page_state(&self) -> String {
        let s = load_settings();
        let spacing = s.spacing_em();
        if hidden() {
            return format!("{{\"t\":\"page\",\"boss\":true,\"font\":{},\"spacing\":{spacing}}}", s.font);
        }
        let Some(book) = current_book() else {
            let dir = plugin_dir().unwrap_or_default();
            return msg_json(&format!(
                "书库是空的：把 txt 小说放进书库文件夹（下方路径），或运行命令“Thief Book: 载入示例小说”。\n{dir}"
            ));
        };
        let pos = load_pos(&book);
        let (n, lines) = match open_book(&book, &s) {
            Ok(pair) => pair,
            Err(_) => return msg_json("书打不开了，可能被移动或删除了，点上方“换”重新选一本。"),
        };
        if lines.is_empty() {
            return msg_json("这本书是空的。");
        }
        let per = s.lines as usize;
        let page_no = n.line_of_offset(&lines, pos) / per;
        let body = n.page(&lines, page_no * per, per);
        let chapter = n.chapter_of(pos).map(|c| c.title.clone()).unwrap_or_else(|| "开篇".into());
        let status = format!(
            "{} · {} · 第 {} / {} 页 · {}%",
            book.trim_end_matches(".txt"),
            chapter,
            page_no + 1,
            Novel::total_pages(&lines, per),
            n.percent(pos),
        );
        let mut js = format!(
            "{{\"t\":\"page\",\"boss\":false,\"font\":{},\"spacing\":{spacing},\"status\":{},\"lines\":[",
            s.font,
            json_str(&status),
        );
        for (i, (text, para_last)) in body.iter().enumerate() {
            if i > 0 {
                js.push(',');
            }
            js.push_str(&format!("[{},{}]", json_str(text), para_last));
        }
        js.push_str("]}");
        js
    }

    fn run(&mut self, act: Act, _ctx: &Invocation) -> Result<Option<String>, Error> {
        match act {
            Act::LibPath => {
                let dir = plugin_dir()?;
                let _ = host::copy_to_clipboard(&dir);
                host::notify(NotificationLevel::Info, "Thief Book 书库", &format!("把 txt 小说放进：\n{dir}\n\n路径已复制到剪贴板。"));
                Ok(None)
            }
            Act::Sample => {
                let path = join(&plugin_dir()?, SAMPLE_NAME);
                host::fs::write_file(&path, SAMPLE)?;
                storage_set(K_BOOK, SAMPLE_NAME);
                save_progress(SAMPLE_NAME, 0, 0);
                let _ = host::reveal_view(VIEW);
                self.sync_ui();
                Ok(None)
            }
            Act::Boss => {
                let now = !hidden();
                storage_set(K_HIDDEN, if now { "1" } else { "0" });
                self.sync_ui();
                Ok(None)
            }
            Act::Font(d) => self.adjust_settings(|s| s.font = (s.font as i32 + d).clamp(12, 32) as u32),
            Act::Lines(d) => self.adjust_settings(|s| s.lines = (s.lines as i32 + d).clamp(1, 12) as u32),
            Act::Spacing(d) => {
                self.adjust_settings(|s| s.spacing_x10 = (s.spacing_x10 as i32 + d).clamp(11, 25) as u32)
            }
            Act::Switch => self.switch_book(),
            Act::Chapters => self.chapters(),
            Act::Jump => self.jump(),
            Act::Next | Act::Prev | Act::NextCh | Act::PrevCh => {
                if hidden() {
                    return Ok(None); // 老板模式下静默忽略，防盲翻错位
                }
                let step: i64 = match act {
                    Act::Next => 1,
                    Act::Prev => -1,
                    Act::NextCh => 2,
                    _ => -2,
                };
                if step.abs() == 1 {
                    self.turn_page(step)
                } else {
                    self.jump_chapter(if step > 0 { Dir::Next } else { Dir::Prev })
                }
            }
        }
    }

    fn adjust_settings(&mut self, f: impl FnOnce(&mut view::Settings)) -> Result<Option<String>, Error> {
        let mut s = load_settings();
        f(&mut s);
        save_settings(&s);
        host::notify(
            NotificationLevel::Info,
            "Thief Book",
            &format!("字号 {} · 每页 {} 行 · 行距 {}em（行宽自适应）", s.font, s.lines, s.spacing_em()),
        );
        self.sync_ui();
        Ok(None)
    }

    fn switch_book(&mut self) -> Result<Option<String>, Error> {
        let books = library()?;
        if books.is_empty() {
            host::notify(NotificationLevel::Warning, "Thief Book", "书库是空的：运行“Thief Book: 书库路径”把 txt 放进去。");
            self.push_state();
            return Ok(None);
        }
        let items: Vec<PickItem> = books
            .iter()
            .map(|b| PickItem::new(b, b.trim_end_matches(".txt")))
            .collect();
        let chosen = host::pick("选择一本书", &items)?;
        let Some(book) = chosen else { return Ok(None) };
        storage_set(K_BOOK, &book);
        let _ = host::reveal_view(VIEW);
        self.sync_ui();
        Ok(None)
    }

    /// 书 + 行布局；书没了时清空当前书并返回 Err（提示用户换书）。
    fn book_ctx(&self) -> Result<(Novel, Vec<Line>), Error> {
        let Some(book) = current_book() else {
            self.push_state();
            return Err("Thief Book：书库是空的，先运行“Thief Book: 书库路径”放书，或“载入示例小说”。".into());
        };
        let s = load_settings();
        match open_book(&book, &s) {
            Ok(pair) => Ok(pair),
            Err(msg) => {
                storage_set(K_BOOK, "");
                host::notify(NotificationLevel::Error, "Thief Book", &msg);
                self.push_state();
                Err(msg.into())
            }
        }
    }

    fn turn_page(&mut self, dir: i64) -> Result<Option<String>, Error> {
        let (n, lines) = self.book_ctx()?;
        if lines.is_empty() {
            return Ok(None);
        }
        let book = current_book().ok_or("没有当前书")?;
        let s = load_settings();
        let per = s.lines as usize;
        let total = Novel::total_pages(&lines, per);
        let pos = load_pos(&book);
        let cur = (n.line_of_offset(&lines, pos) / per) as i64;
        let next = (cur + dir).clamp(0, total as i64 - 1) as usize;
        let new_pos = lines[next * per].start;
        save_progress(&book, new_pos, n.percent(new_pos));
        self.sync_ui();
        Ok(None)
    }

    fn jump_chapter(&mut self, dir: Dir) -> Result<Option<String>, Error> {
        let (n, _lines) = self.book_ctx()?;
        let book = current_book().ok_or("没有当前书")?;
        let pos = load_pos(&book);
        let ch = match dir {
            Dir::Next => n.next_chapter(pos),
            Dir::Prev => n.prev_chapter(pos),
        };
        let Some(ch) = ch else {
            host::notify(NotificationLevel::Info, "Thief Book", if dir == Dir::Next { "已经是最后一章" } else { "已经是第一章" });
            return Ok(None);
        };
        save_progress(&book, ch.start, n.percent(ch.start));
        self.sync_ui();
        Ok(None)
    }

    fn jump(&mut self) -> Result<Option<String>, Error> {
        if hidden() {
            return Ok(None); // 老板模式下不开对话框
        }
        let (n, lines) = self.book_ctx()?;
        let book = current_book().ok_or("没有当前书")?;
        let s = load_settings();
        let per = s.lines as usize;
        let total = Novel::total_pages(&lines, per);
        let cur = n.line_of_offset(&lines, load_pos(&book)) / per + 1;
        let Some(v) = host::input(&format!("跳到第几页？（共 {total} 页）"), Some(&cur.to_string()))? else {
            return Ok(None);
        };
        let Ok(p) = v.trim().parse::<usize>() else {
            host::notify(NotificationLevel::Warning, "Thief Book", &format!("“{v}”不是页码"));
            return Ok(None);
        };
        let page = p.clamp(1, total);
        let new_pos = lines[(page - 1) * per].start;
        save_progress(&book, new_pos, n.percent(new_pos));
        self.sync_ui();
        Ok(None)
    }

    fn chapters(&mut self) -> Result<Option<String>, Error> {
        if hidden() {
            return Ok(None);
        }
        let (n, _lines) = self.book_ctx()?;
        let book = current_book().ok_or("没有当前书")?;
        if n.chapters.is_empty() {
            host::notify(NotificationLevel::Info, "Thief Book", "这本书没有识别到章节标题（第X章/卷/节/回）。");
            return Ok(None);
        }
        let items: Vec<PickItem> = n
            .chapters
            .iter()
            .map(|c| PickItem::new(&c.title.to_string(), &format!("{}%", n.percent(c.start))))
            .collect();
        let chosen = host::pick("章节目录", &items)?;
        let Some(title) = chosen else { return Ok(None) };
        let Some(ch) = n.chapters.iter().find(|c| c.title == title) else { return Ok(None) };
        save_progress(&book, ch.start, n.percent(ch.start));
        self.sync_ui();
        Ok(None)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Next,
    Prev,
}

impl Plugin for ThiefBook {
    fn run_command(&mut self, id: &str, ctx: &Invocation, _arg: Option<&str>) -> Result<Option<String>, Error> {
        let act = match id {
            "prev-page" => Act::Prev,
            "next-page" => Act::Next,
            "boss-key" => Act::Boss,
            "next-chapter" => Act::NextCh,
            "prev-chapter" => Act::PrevCh,
            "jump" => Act::Jump,
            "chapters" => Act::Chapters,
            "switch-book" => Act::Switch,
            "font-bigger" => Act::Font(1),
            "font-smaller" => Act::Font(-1),
            "lines-more" => Act::Lines(1),
            "lines-less" => Act::Lines(-1),
            "spacing-more" => Act::Spacing(1),
            "spacing-less" => Act::Spacing(-1),
            "library-path" => Act::LibPath,
            "load-sample" => Act::Sample,
            other => return Err(format!("Thief Book：未知命令 {other}").into()),
        };
        self.run(act, ctx)
    }
}

impl Views for ThiefBook {
    /// web 视图的内容由页面（web/index.html）渲染，状态经 push_state 推送；
    /// render 仅在 IDE 需要占位时被调用。
    fn render(
        &mut self,
        _view: &str,
        _viewport: Option<openidea_plugin::views::Viewport>,
        _ctx: &Invocation,
    ) -> Result<View, Error> {
        Ok(View::message(""))
    }

    fn handle(&mut self, _view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        // 面板（web 页面）发来的消息
        let ViewEvent::Message(msg) = event else { return Ok(()) };
        let act = match msg.as_str() {
            "hello" => {
                self.push_state();
                return Ok(());
            }
            "next" => Act::Next,
            "prev" => Act::Prev,
            "boss" => Act::Boss,
            "chapters" => Act::Chapters,
            "switch" => Act::Switch,
            "font+" => Act::Font(1),
            "font-" => Act::Font(-1),
            "lines+" => Act::Lines(1),
            "lines-" => Act::Lines(-1),
            "spacing+" => Act::Spacing(1),
            "spacing-" => Act::Spacing(-1),
            _ => return Ok(()),
        };
        self.run(act, ctx).map(|_| ())
    }
}

register_plugin!(ThiefBook, views);

/// 示例小说（占位文本，仅为演示排版/翻章/进度）。
const SAMPLE: &str = r#"第一章 客栈夜雨
雨是从黄昏下起的，先细后密，到三更天，檐角的水声已经连成了线。
李慕白推门进来的时候，店小二正打算上板。他抖落蓑衣上的水，要了一间上房、一壶热酒，两个小菜。
窗外的官道上空无一人，只有驿灯在雨里晕开一团昏黄。
他斟了一杯，没喝，只是听着雨。
这样的雨夜，适合想一些往事，也适合忘掉一些。
酒凉透的时候，楼梯响了。脚步很轻，却踩得很准，每一步都落在梯子的实处。
来人一身青衣，斗笠也没摘，在他对面坐了下来，自顾自倒了一杯酒。
"李二爷的剑，听说三个月没出鞘了。"
"出鞘要见血，"李慕白说，"我不想见。"
"可有人想见你的血。"青衣人把杯中酒一口饮尽，"北边来的，一共七个人，现在已在城外。"
雨声忽然密了起来，像是有很多脚，从很远的地方跑过来。

第二章 故人
第二天雨停了，官道上的泥还没干，车轮的印子一道压着一道，一直伸到天边。
李慕白没有走城门，他沿着城墙根走了半里地，从一个塌了半边的水门出去。
城外十里的破庙里，果然有人在等。
七个人，都很年轻，只有为首的那个上了年纪，鬓角已经白了。
"二十年了，"白鬓人说，"你师父死的那年，你也是这样站在雨里。"
"你们记得倒清楚。"
"他欠门里的东西，总要有人还。"
李慕白把手按在剑柄上，忽然觉得剑很重。二十年前它也很重，那年他十五岁。
"东西在我这里，"他说，"人也可以给你们。剑不行。"
白鬓人叹了口气，往后退了半步。
七把刀同时出了鞘。

第三章 长街
后来镇上的人说，那天早上听见庙里有金铁交鸣的声音，密得像另一场雨。
也有人说，其实没有声音，只有一大群鸟从庙顶飞起来，遮了半个天。
日头升到一竿高的时候，李慕白一个人走出来，蓑衣搭在肩上，剑还在腰间，只是鞘口多了一道新崩的缺口。
他在镇口的面摊上要了两碗阳春面，一碗多的那碗推到对面。
对面没有人。
"你不吃？"摊主探头看了一眼空位。
"给她留的，"李慕白说，"她马上就到。"
摊主笑了笑，没再问。江湖上的怪人他见得多了，只要给钱，都是好客人。
长街尽头传来马蹄声，很急，由远及近。
李慕白低头吃面，吃得很慢。
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    fn setup(mock: &MockHost, book: &str, text: &str) {
        let dir = host::fs::plugin_dir().unwrap();
        mock.add_external_file(&join(&dir, book), text);
    }

    #[test]
    fn sample_switches_page_turns_and_persists() {
        let mock = MockHost::install().with_capabilities(&["storage"]);
        setup(&mock, "书甲.txt", "第一章 起\n正文甲。\n第二章 承\n正文乙。");

        // 换书：pick 返回书名 key
        mock.answer_next_prompt(Some(&["书甲.txt"]));
        ThiefBook.run_command("switch-book", &mock.invocation(), None).unwrap();
        assert_eq!(storage_get(K_BOOK).as_deref(), Some("书甲.txt"));

        // 下一页：书只有一页，页码 clamp 不越界，pos = 行0 起点
        ThiefBook.run_command("next-page", &mock.invocation(), None).unwrap();
        let (n, lines) = {
            let s = load_settings();
            open_book("书甲.txt", &s).unwrap()
        };
        assert_eq!(load_pos("书甲.txt"), lines[0].start);
        assert_eq!(mock.storage().get(&pct_key("书甲.txt")), Some(&"0".to_string()));
        assert_eq!(n.chapters.len(), 2);

        // 跳章
        ThiefBook.run_command("next-chapter", &mock.invocation(), None).unwrap();
        assert_eq!(load_pos("书甲.txt"), n.chapters[1].start);
        ThiefBook.run_command("prev-chapter", &mock.invocation(), None).unwrap();
        assert_eq!(load_pos("书甲.txt"), n.chapters[0].start);
    }

    #[test]
    fn boss_key_hides_restores_and_blocks_turning() {
        let mock = MockHost::install().with_capabilities(&["storage"]);
        setup(&mock, "书乙.txt", "第一章 起\n内容。");
        mock.answer_next_prompt(Some(&["书乙.txt"]));
        ThiefBook.run_command("switch-book", &mock.invocation(), None).unwrap();
        let n0 = mock.view_messages().len();

        // 老板键 → hidden，且向页面推 boss:true
        ThiefBook.run_command("boss-key", &mock.invocation(), None).unwrap();
        assert!(hidden());
        assert!(mock.status_items().get(ITEM).is_none(), "老板模式状态栏应无小说项");
        let pushed = mock.view_messages();
        assert!(pushed.len() > n0, "老板键应推送新状态");
        assert!(pushed.last().unwrap().1.contains("\"boss\":true"), "应推送老板屏状态：{pushed:?}");

        // 隐藏期间翻页被忽略
        let before = load_pos("书乙.txt");
        ThiefBook.run_command("next-page", &mock.invocation(), None).unwrap();
        assert_eq!(load_pos("书乙.txt"), before);

        // 再按恢复：boss:false 且状态栏项回来
        ThiefBook.run_command("boss-key", &mock.invocation(), None).unwrap();
        assert!(!hidden());
        assert!(mock.status_items().contains_key(ITEM));
        let pushed = mock.view_messages();
        assert!(pushed.last().unwrap().1.contains("\"boss\":false"));
        assert!(pushed.last().unwrap().1.contains("内容"), "恢复后应推送正文");
    }

    #[test]
    fn webview_messages_drive_actions_and_push_state() {
        let mock = MockHost::install().with_capabilities(&["storage"]);
        // 章节行 + 5 个段落 → 6 段；每页 5 段 → 两页，next 才有得翻
        let mut body = String::from("第一章 起\n");
        for _ in 0..5 {
            body.push_str(&"好".repeat(50));
            body.push('\n');
        }
        setup(&mock, "书丁.txt", &body);
        mock.answer_next_prompt(Some(&["书丁.txt"]));
        ThiefBook.run_command("switch-book", &mock.invocation(), None).unwrap();
        mock.view_messages().clear();

        // hello → 推送当前页
        ThiefBook.handle("reader", ViewEvent::Message("hello".into()), &mock.invocation()).unwrap();
        let pushed = mock.view_messages();
        assert!(pushed.last().unwrap().1.contains("\"t\":\"page\""), "hello 应收到页面状态：{pushed:?}");

        // next 消息推进页首并推送新状态
        let before = load_pos("书丁.txt");
        ThiefBook.handle("reader", ViewEvent::Message("next".into()), &mock.invocation()).unwrap();
        assert!(load_pos("书丁.txt") > before, "next 消息应推进页首");
        assert!(mock.view_messages().len() > pushed.len(), "翻页后应再次推送");

        // prev 退回；boss 进入老板模式；未知消息忽略
        ThiefBook.handle("reader", ViewEvent::Message("prev".into()), &mock.invocation()).unwrap();
        assert_eq!(load_pos("书丁.txt"), before);
        ThiefBook.handle("reader", ViewEvent::Message("boss".into()), &mock.invocation()).unwrap();
        assert!(hidden());
        ThiefBook.handle("reader", ViewEvent::Message("whatever".into()), &mock.invocation()).unwrap();
        assert!(hidden());
    }

    #[test]
    fn settings_commands_adjust_and_persist() {
        let mock = MockHost::install().with_capabilities(&["storage"]);
        ThiefBook.run_command("font-bigger", &mock.invocation(), None).unwrap();
        assert_eq!(load_settings().font, 15);
        ThiefBook.run_command("lines-more", &mock.invocation(), None).unwrap();
        assert_eq!(load_settings().lines, 6);
        ThiefBook.run_command("spacing-more", &mock.invocation(), None).unwrap();
        assert_eq!(load_settings().spacing_x10, 16);
        // 边界：行数减到底
        for _ in 0..20 {
            ThiefBook.run_command("lines-less", &mock.invocation(), None).unwrap();
        }
        assert_eq!(load_settings().lines, 1);
    }

    #[test]
    fn jump_by_page_number() {
        let mock = MockHost::install().with_capabilities(&["storage"]);
        // 10 个段落（每段 20 字）、每页 2 段 → 共 5 页
        setup(&mock, "书丙.txt", &format!("{}\n", "好".repeat(20)).repeat(10));
        storage_set(K_SETTINGS, &view::Settings { font: 14, lines: 2, spacing_x10: 15 }.encode());
        mock.answer_next_prompt(Some(&["书丙.txt"]));
        ThiefBook.run_command("switch-book", &mock.invocation(), None).unwrap();

        let s = load_settings();
        let per = s.lines as usize;
        let (_n, lines) = open_book("书丙.txt", &s).unwrap();
        let total = Novel::total_pages(&lines, per);
        assert_eq!(total, 5);

        // 跳到第 3 页
        mock.answer_next_prompt(Some(&["3"]));
        ThiefBook.run_command("jump", &mock.invocation(), None).unwrap();
        assert_eq!(load_pos("书丙.txt"), lines[(3 - 1) * per].start);

        // 越界页码 clamp 到最后一页
        mock.answer_next_prompt(Some(&["99"]));
        ThiefBook.run_command("jump", &mock.invocation(), None).unwrap();
        assert_eq!(load_pos("书丙.txt"), lines[(total - 1) * per].start);
    }

    #[test]
    fn library_path_notifies_and_copies() {
        let mock = MockHost::install().with_capabilities(&["storage", "clipboard"]);
        ThiefBook.run_command("library-path", &mock.invocation(), None).unwrap();
        assert_eq!(mock.notifications().len(), 1);
        assert!(mock.clipboard().is_some());
    }
}
