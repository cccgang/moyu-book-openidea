//! 书籍文本模型：解码 → 段落 → 显示行 → 页 → 章节。
//!
//! 全部偏移量按字符计（内部存 `Vec<char>`），与显示宽度（每行字数）一致。

use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};

/// 宿主 fs 单次读取上限 16 MiB，这里同样设防。
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chapter {
    /// 章首的字符偏移。
    pub start: usize,
    pub title: String,
}

/// 一条显示行：字符区间 `start..end`（end 不含），`para_last` 标记段落末行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub para_last: bool,
}

pub struct Novel {
    pub chars: Vec<char>,
    /// 非空段落的字符区间，按出现顺序。
    pub paras: Vec<(usize, usize)>,
    pub chapters: Vec<Chapter>,
}

/// 按字节识别编码并解码：UTF-8/16 BOM → 严格 UTF-8 → GB18030（容错）。
/// 返回（解码文本，编码名）。
pub fn decode(bytes: &[u8]) -> Result<(String, &'static str), String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("文件约 {} MiB，超过 16 MiB 上限", bytes.len() >> 20));
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok((String::from_utf8_lossy(&bytes[3..]).into_owned(), "UTF-8"));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (s, _) = UTF_16LE.decode_without_bom_handling(&bytes[2..]);
        return Ok((s.into_owned(), "UTF-16LE"));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _) = UTF_16BE.decode_without_bom_handling(&bytes[2..]);
        return Ok((s.into_owned(), "UTF-16BE"));
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok((s.to_owned(), "UTF-8"));
    }
    let (s, _) = GB18030.decode_without_bom_handling(bytes);
    Ok((s.into_owned(), "GB18030"))
}

const NUMERALS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', //
    '零', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千', '万', '两', '〇',
];
const CHAPTER_SUFFIX: &[char] = &['章', '卷', '节', '回', '部', '集', '篇'];

/// 章节标题启发式：`第X章/卷/节/回/部/集/篇`（X 为数字或中文数字，整行 ≤ 24 字，
/// 排除以“第一章…”开头的叙事行），或 `Chapter N`。
pub fn is_chapter_title(s: &str) -> bool {
    if s.chars().count() > 24 {
        return false;
    }
    match s.strip_prefix('第') {
        Some(rest) => {
            let n = rest.chars().take_while(|c| NUMERALS.contains(c)).count();
            if n == 0 {
                return false;
            }
            match rest.chars().nth(n) {
                Some(c) if CHAPTER_SUFFIX.contains(&c) => true,
                _ => false,
            }
        }
        None => {
            let lower = s.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("chapter ") {
                rest.chars().next().is_some_and(|c| c.is_ascii_digit())
            } else {
                false
            }
        }
    }
}

impl Novel {
    /// 由解码文本构建书籍模型；`\r\n`/`\r` 统一为换行处理，空行不产生段落。
    pub fn parse(raw: &str) -> Novel {
        let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
        let chars: Vec<char> = normalized.chars().collect();

        let mut paras = Vec::new();
        let mut chapters = Vec::new();
        let mut char_off = 0usize;
        for seg in normalized.split('\n') {
            let seg_chars = seg.chars().count();
            let lead = seg.len() - seg.trim_start().len();
            let lead_chars = seg[..lead].chars().count();
            let trimmed = seg.trim();
            if !trimmed.is_empty() {
                let start = char_off + lead_chars;
                let end = start + trimmed.chars().count();
                paras.push((start, end));
                if is_chapter_title(trimmed) {
                    chapters.push(Chapter { start, title: trimmed.to_owned() });
                }
            }
            char_off += seg_chars + 1; // +1 为换行符
        }

        Novel { chars, paras, chapters }
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end.min(self.chars.len())].iter().collect()
    }

    /// 显示行 = txt 的一个段落（自然段）。长段超出面板宽度时由页面 CSS 自然折行，
    /// 段落装不下时页面内滚动，不在这里按宽度切块。
    pub fn build_lines(&self) -> Vec<Line> {
        self.paras
            .iter()
            .map(|&(ps, pe)| Line { start: ps, end: pe, para_last: true })
            .collect()
    }

    /// 偏移所在显示行下标（取 `start <= offset` 的最后一行）。
    pub fn line_of_offset(&self, lines: &[Line], offset: usize) -> usize {
        lines.partition_point(|l| l.start <= offset).saturating_sub(1)
    }

    /// 一页正文：从显示行 `first_line` 起最多 `per_page` 行，返回（文本，是否段末）。
    pub fn page(&self, lines: &[Line], first_line: usize, per_page: usize) -> Vec<(String, bool)> {
        let a = first_line.min(lines.len());
        let b = (a + per_page).min(lines.len());
        lines[a..b].iter().map(|l| (self.slice(l.start, l.end), l.para_last)).collect()
    }

    /// 总页数。
    pub fn total_pages(lines: &[Line], per_page: usize) -> usize {
        lines.len().div_ceil(per_page.max(1))
    }

    /// 阅读进度百分比（0..=100）。
    pub fn percent(&self, offset: usize) -> u32 {
        if self.chars.is_empty() {
            return 0;
        }
        ((offset.min(self.chars.len()) as u64 * 100) / self.chars.len() as u64) as u32
    }

    /// 当前偏移所在章（全书无章节标记时 None）。
    pub fn chapter_of(&self, offset: usize) -> Option<&Chapter> {
        let idx = self.chapters.partition_point(|c| c.start <= offset).checked_sub(1)?;
        self.chapters.get(idx)
    }

    /// 下一章（偏移之后第一个章首）；已是最后一章则 None。
    pub fn next_chapter(&self, offset: usize) -> Option<&Chapter> {
        self.chapters.iter().find(|c| c.start > offset)
    }

    /// 上一章：先退到本章章首，再取前一章（连续按两次回到上一章开头）。
    /// 已在本章开头且还有前一章时返回前一章。
    pub fn prev_chapter(&self, offset: usize) -> Option<&Chapter> {
        let cur = self.chapter_of(offset);
        match cur {
            Some(c) if c.start < offset => Some(c),
            _ => {
                let idx = self.chapters.partition_point(|c| c.start <= offset).checked_sub(1)?;
                if idx == 0 {
                    None
                } else {
                    self.chapters.get(idx - 1)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_gbk_and_utf16() {
        // UTF-8
        let (s, enc) = decode("你好\n世界".as_bytes()).unwrap();
        assert_eq!((s.as_str(), enc), ("你好\n世界", "UTF-8"));

        // UTF-8 BOM
        let (s, enc) = decode(&[b"\xEF\xBB\xBF", "你好".as_bytes()].concat()).unwrap();
        assert_eq!((s.as_str(), enc), ("你好", "UTF-8"));

        // GBK（用 encoding_rs 把中文编成 GB18030 字节）
        let (gb_bytes, _, _) = encoding_rs::GB18030.encode("第三章 剑气长江\n李慕白握紧长剑。");
        let (s, enc) = decode(&gb_bytes).unwrap();
        assert_eq!(enc, "GB18030");
        assert!(s.contains("剑气长江"));

        // UTF-16LE BOM
        let mut u16le = vec![0xFF, 0xFE];
        for u in "你好".encode_utf16() {
            u16le.extend_from_slice(&u.to_le_bytes());
        }
        let (s, enc) = decode(&u16le).unwrap();
        assert_eq!((s.as_str(), enc), ("你好", "UTF-16LE"));

        // 超限拒绝
        assert!(decode(&vec![0x61; MAX_BYTES + 1]).is_err());
    }

    #[test]
    fn parses_paragraphs_and_chapters() {
        let n = Novel::parse(
            "第一章 起势\r\n李慕白握紧长剑。\r\n\r\n第二章 出山\n那人冷笑。\n第二天他又去了。\n第一章不该匹配这句话因为太长了吗第一章还在行首不过无数字",
        );
        assert_eq!(n.paras.len(), 6);
        assert_eq!(n.chapters.len(), 2);
        assert_eq!(n.chapters[0].title, "第一章 起势");
        assert_eq!(n.chapters[1].title, "第二章 出山");
        // 行首"第二天"不是章节
        assert!(!is_chapter_title("第二天他又去了。"));
        assert!(is_chapter_title("第一百二十三回 大战"));
        assert!(is_chapter_title("Chapter 12"));
        assert!(!is_chapter_title("Chapter abc"));
        assert!(is_chapter_title("第三章")); // 光秃秃的章名也成立
    }

    #[test]
    fn one_line_per_paragraph() {
        // 显示行 = txt 段落；空行不算；长段整段一行（折行交给页面 CSS）
        let n = Novel::parse("第一行短\n第二行很长很长很长很长很长很长很长很长很长\n\n第三行");
        let lines = n.build_lines();
        assert_eq!(lines.len(), 3);
        assert_eq!(n.slice(lines[1].start, lines[1].end), "第二行很长很长很长很长很长很长很长很长很长");
        assert!(lines.iter().all(|l| l.para_last));

        // 每页 2 段 → 2 页
        let p0 = n.page(&lines, 0, 2);
        assert_eq!(p0.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["第一行短", "第二行很长很长很长很长很长很长很长很长很长"]);
        let p1 = n.page(&lines, 2, 2);
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].0, "第三行");
        assert_eq!(Novel::total_pages(&lines, 2), 2);
    }

    #[test]
    fn line_of_offset_and_nav_math() {
        let n = Novel::parse("零一二三四五六七八九\nabc");
        let lines = n.build_lines();
        let per = 1usize;
        let total_pages = Novel::total_pages(&lines, per);

        // offset 0 → 段 0；offset 在 "abc" → 段 1
        assert_eq!(n.line_of_offset(&lines, 0), 0);
        assert_eq!(n.line_of_offset(&lines, 4), 0);
        assert_eq!(n.line_of_offset(&lines, 12), 1);
        assert_eq!(total_pages, 2);

        // 页起点 = 页号 * per，对应段的 start
        let next_first = 1 * per;
        assert_eq!(lines[next_first].start, 11); // "abc" 起点（10 字符段 + \n）
    }

    #[test]
    fn chapter_navigation() {
        let n = Novel::parse("第一章 起\n正文甲。\n\n第二章 承\n正文乙。\n\n第三章 转\n正文丙。");
        assert_eq!(n.chapters.len(), 3);
        let off_ch2 = n.chapters[1].start;

        // 在第二章中间 → 下一章是第三章；上一章回到第二章开头；再按一次回到第一章
        let mid_ch2 = off_ch2 + 3;
        assert_eq!(n.chapter_of(mid_ch2).unwrap().title, "第二章 承");
        assert_eq!(n.next_chapter(mid_ch2).unwrap().title, "第三章 转");
        assert_eq!(n.prev_chapter(mid_ch2).unwrap().title, "第二章 承");
        assert_eq!(n.prev_chapter(off_ch2).unwrap().title, "第一章 起");
        // 第一章开头再往上是 None
        assert!(n.prev_chapter(n.chapters[0].start).is_none());
    }

    #[test]
    fn percent_progress() {
        let n = Novel::parse("零一二三四五六七八九");
        assert_eq!(n.percent(0), 0);
        assert_eq!(n.percent(5), 50);
        assert_eq!(n.percent(999), 100);
    }
}
