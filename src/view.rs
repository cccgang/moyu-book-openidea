//! 阅读面板的设置项（storage 持久化为 `font,lines,spacing_x10` 逗号串）。
//! 面板本体是 web 视图（web/index.html），插件经 post_view_message 推 JSON 状态。
//! 每行宽度不再手工设定：页面实测自身像素宽度后上报（`w=<em 数>`，存 storage 键 `cap`）。

/// 设置项。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// 字号 px（12..=32）。
    pub font: u32,
    /// 每页显示行数（1..=12）。
    pub lines: u32,
    /// 行距 ×10（11..=25 即 1.1em..2.5em；默认 15 = 1.5em，即字高一半的行间隔）。
    pub spacing_x10: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { font: 14, lines: 5, spacing_x10: 15 }
    }
}

impl Settings {
    /// 行距，如 `1.5`。
    pub fn spacing_em(&self) -> String {
        format!("{:.1}", self.spacing_x10 as f32 / 10.0)
    }

    pub fn encode(&self) -> String {
        format!("{},{},{}", self.font, self.lines, self.spacing_x10)
    }

    pub fn decode(s: &str) -> Settings {
        let d = Settings::default();
        let mut it = s.split(',');
        let num = |it: &mut std::str::Split<'_, char>, fallback: u32| -> u32 {
            it.next().and_then(|v| v.trim().parse().ok()).unwrap_or(fallback)
        };
        Settings {
            font: num(&mut it, d.font).clamp(12, 32),
            lines: num(&mut it, d.lines).clamp(1, 12),
            spacing_x10: num(&mut it, d.spacing_x10).clamp(11, 25),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_clamp() {
        let s = Settings { font: 16, lines: 6, spacing_x10: 18 };
        assert_eq!(Settings::decode(&s.encode()), s);
        // 越界值钳到边界
        assert_eq!(Settings::decode("999,0,999"), Settings { font: 32, lines: 1, spacing_x10: 25 });
        assert_eq!(Settings::decode("garbage"), Settings::default());
        assert_eq!(Settings::decode("16,6"), Settings { font: 16, lines: 6, spacing_x10: 15 });
        assert_eq!(Settings::default().spacing_em(), "1.5");
    }
}
