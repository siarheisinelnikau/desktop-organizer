//! Modeless proposal conversation. Hidden windows receive no display updates or timers.
use super::*;
const CLASS: PCWSTR = w!("DesktopOrganizer.Conversation.v1");
const HISTORY: i32 = 1;
const INPUT: i32 = 2;
const SEND: i32 = AI_CHAT_SEND as i32;
const RESET: i32 = FORGET_CHOICES as i32;
const STAGE: i32 = 4;
#[derive(Default)]
pub(super) struct Conversation {
    pub window: HWND,
    history: String,
    answer: String,
    pub stage: String,
    pub guidance: String,
    pass: u8,
}
impl Conversation {
    pub unsafe fn show(&mut self, owner: HWND, font: HFONT) {
        unsafe {
            if self.window.is_null() || IsWindow(self.window) == 0 {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(procedure),
                    hInstance: GetModuleHandleW(null()),
                    lpszClassName: CLASS,
                    hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                    ..zeroed()
                };
                RegisterClassW(&class);
                self.window = CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_COMPOSITED,
                    CLASS,
                    wide(tr("AI conversation")).as_ptr(),
                    WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    850,
                    660,
                    owner,
                    null_mut(),
                    GetModuleHandleW(null()),
                    null(),
                );
                for (id, class, style, text) in [
                    (STAGE, w!("STATIC"), 0, String::new()),
                    (
                        HISTORY,
                        w!("EDIT"),
                        ES_MULTILINE as u32
                            | ES_READONLY as u32
                            | ES_AUTOVSCROLL as u32
                            | WS_VSCROLL,
                        String::new(),
                    ),
                    (
                        INPUT,
                        w!("EDIT"),
                        ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | WS_VSCROLL,
                        String::new(),
                    ),
                    (
                        SEND,
                        w!("BUTTON"),
                        BS_OWNERDRAW as u32,
                        tr("Send & regroup").to_owned(),
                    ),
                    (
                        RESET,
                        w!("BUTTON"),
                        BS_OWNERDRAW as u32,
                        tr("Forget grouping choices").to_owned(),
                    ),
                ] {
                    let child = CreateWindowExW(
                        if id == INPUT || id == HISTORY {
                            WS_EX_CLIENTEDGE
                        } else {
                            0
                        },
                        class,
                        wide(text).as_ptr(),
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | style,
                        0,
                        0,
                        0,
                        0,
                        self.window,
                        id as usize as HMENU,
                        GetModuleHandleW(null()),
                        null(),
                    );
                    SendMessageW(child, WM_SETFONT, font as usize, 0);
                    style_dialog_control(child);
                    if id == INPUT {
                        SendMessageW(child, EM_SETLIMITTEXT, 2000, 0);
                    }
                    if id == HISTORY {
                        SendMessageW(child, EM_SETLIMITTEXT, 80 * 1024, 0);
                    }
                }
                layout(self.window);
            }
            self.prepare_capture();
            if CAPTURE_DPI.load(Ordering::Relaxed) == 0 {
                ShowWindow(self.window, SW_SHOWNORMAL);
                SetForegroundWindow(self.window);
            }
        }
    }
    pub fn push(&mut self, speaker: &str, text: &str) {
        self.history
            .push_str(&format!("{}\r\n{}\r\n\r\n", tr(speaker), text));
        if self.history.len() > 48 * 1024 {
            let mut start = self.history.len() - 40 * 1024;
            while !self.history.is_char_boundary(start) {
                start += 1;
            }
            self.history = self.history[start..].to_owned();
        }
    }
    pub fn begin(&mut self, count: usize, guidance: &str) {
        self.answer.clear();
        self.pass = 0;
        self.push(
            "You",
            if guidance.is_empty() {
                tr("Organize the selected items by purpose.")
            } else {
                guidance
            },
        );
        self.stage = trf!("Reading program details: 0 / {}", count);
    }
    pub fn progress(&mut self, progress: ai::Progress) {
        match progress {
            ai::Progress::Inspecting(done, total, name) => {
                self.stage = trf!("Reading program details: {} / {} · {}", done, total, name)
            }
            ai::Progress::Requesting(pass) => {
                self.pass = pass;
                self.answer.clear();
                self.stage = if pass == 1 {
                    tr("Waiting for the model's first response…").into()
                } else {
                    tr("Refining broad folders · waiting for the model…").into()
                };
            }
            ai::Progress::Answer(pass, text, count) => {
                self.pass = pass;
                self.answer = text;
                self.stage = trf!("Receiving answer · pass {} · {} characters", pass, count);
            }
            ai::Progress::Validating => self.stage = tr("Checking the proposed folders…").into(),
        }
    }
    pub fn finish(&mut self, proposal: &ai::Proposal) {
        self.answer.clear();
        let summary = if proposal.summary.is_empty() {
            proposal
                .groups
                .iter()
                .map(|g| format!("{} — {}", g.name, g.item_ids.len()))
                .collect::<Vec<_>>()
                .join("\r\n")
        } else {
            proposal.summary.clone()
        };
        self.push("AI", &summary);
        self.stage = tr("Proposal ready. Review it before applying.").into();
    }
    pub fn answer_error(&mut self, message: &str) {
        self.answer.clear();
        self.push("Error", message);
        self.stage = tr("Request failed. The existing preview is preserved.").into();
    }
    pub unsafe fn input(&self) -> String {
        unsafe { text(GetDlgItem(self.window, INPUT)) }
    }
    pub unsafe fn refresh(&self) {
        unsafe {
            if self.window.is_null() || IsWindowVisible(self.window) == 0 {
                return;
            }
            self.prepare_capture();
        }
    }
    pub unsafe fn prepare_capture(&self) {
        unsafe {
            let display = if self.answer.is_empty() {
                self.history.clone()
            } else {
                format!("{}{}\r\n{}", self.history, tr("AI"), self.answer)
            };
            let history = GetDlgItem(self.window, HISTORY);
            if text(history) != display {
                SetWindowTextW(
                    history,
                    wide(display.replace("\r\n", "\n").replace('\n', "\r\n")).as_ptr(),
                );
                SendMessageW(history, EM_SETSEL, usize::MAX, -1);
                SendMessageW(history, EM_SCROLLCARET, 0, 0);
            }
            SetWindowTextW(GetDlgItem(self.window, STAGE), wide(&self.stage).as_ptr());
        }
    }
    pub unsafe fn busy(&self, busy: bool) {
        unsafe {
            if !self.window.is_null() {
                EnableWindow(GetDlgItem(self.window, SEND), (!busy) as i32);
                EnableWindow(GetDlgItem(self.window, RESET), (!busy) as i32);
            }
        }
    }
    pub unsafe fn clear_input(&self) {
        unsafe {
            SetWindowTextW(GetDlgItem(self.window, INPUT), w!(""));
        }
    }
}
unsafe fn text(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd).clamp(0, 80 * 1024);
        let mut buffer = vec![0u16; length as usize + 1];
        GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

impl App {
    pub(super) unsafe fn begin_ai(&mut self, guidance: &str) -> Result<()> {
        unsafe {
            ensure!(
                self.task == 0,
                tr("Wait for the current background operation")
            );
            ensure!(
                !self.ai_settings_dirty(),
                tr("Save your AI settings before asking for groups.")
            );
            ensure!(
                self.checked(CONSENT),
                tr("Check the explicit metadata sharing consent first")
            );
            self.scope = self.selected_items(AI_ITEMS);
            ensure!(
                !self.scope.is_empty() && self.scope.len() <= crate::model::MAX_AI_ITEMS,
                tr(
                    "AI supports up to 200 files at once. Use Suggest folders for all files or select a smaller batch."
                )
            );
            let mut combined = self.conversation.guidance.clone();
            if !guidance.trim().is_empty() {
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(guidance.trim());
            }
            let combined: String = combined
                .chars()
                .rev()
                .take(3800)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            self.worker.submit(Job::Ai {
                base: self.layout.settings.ai_base_url.clone(),
                model: self.layout.settings.ai_model.clone(),
                key: self.layout.settings.protected_api_key.clone(),
                items: self.scope.clone(),
                corrections: self.layout.corrections.clone(),
                guidance: format!(
                    "Write folder names and the public summary in {}.\n{}",
                    self.layout.settings.language.name(),
                    combined
                ),
            })?;
            self.plan = None;
            self.task = 2;
            self.conversation.guidance = combined;
            self.layout.settings.ai_grouping_guidance = self.conversation.guidance.clone();
            self.changed();
            self.conversation.begin(self.scope.len(), guidance);
            self.conversation.show(self.hwnd, self.font);
            self.conversation.busy(true);
            self.sync_ai_activity();
            Ok(())
        }
    }
    pub(super) unsafe fn sync_ai_activity(&self) {
        unsafe {
            self.set(AI_ACTIVITY, &self.conversation.stage);
            self.conversation.refresh();
            self.conversation.busy(self.task != 0);
        }
    }
    pub(super) fn remember_choice(&mut self, ids: &[u64], name: &str) {
        let items: Vec<_> = self
            .layout
            .items
            .iter()
            .filter(|i| ids.contains(&i.id))
            .cloned()
            .collect();
        crate::memory::remember(&mut self.layout.corrections, &items, name);
    }
}
unsafe fn layout(hwnd: HWND) {
    unsafe {
        AUX_LAYOUTS.fetch_add(1, Ordering::Relaxed);
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        let dpi = window_dpi(hwnd);
        let s = |v| scale(v, dpi);
        for (id, x, y, w, h) in [
            (STAGE, s(24), s(62), r.right - s(48), s(42)),
            (HISTORY, s(24), s(108), r.right - s(48), r.bottom - s(270)),
            (INPUT, s(24), r.bottom - s(150), r.right - s(48), s(80)),
            (SEND, r.right - s(224), r.bottom - s(56), s(200), s(34)),
            (RESET, s(24), r.bottom - s(56), s(260), s(34)),
        ] {
            SetWindowPos(
                GetDlgItem(hwnd, id),
                null_mut(),
                x,
                y,
                w.max(1),
                h.max(1),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_DRAWITEM => theme::draw_control(lp as *const DRAWITEMSTRUCT),
            WM_CLOSE => {
                ShowWindow(hwnd, SW_HIDE);
                0
            }
            WM_COMMAND if word(wp) == SEND as u16 || word(wp) == RESET as u16 => {
                PostMessageW(GetWindow(hwnd, GW_OWNER), WM_COMMAND, word(wp) as usize, 0);
                0
            }
            WM_SIZE => {
                layout(hwnd);
                0
            }
            WM_GETMINMAXINFO => {
                (*(lp as *mut MINMAXINFO)).ptMinTrackSize = POINT {
                    x: scale(600, window_dpi(hwnd)),
                    y: scale(500, window_dpi(hwnd)),
                };
                0
            }
            WM_ERASEBKGND => 1,
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
                let dc = wp as HDC;
                SetTextColor(dc, theme::color(31, 44, 63));
                SetBkColor(dc, theme::color(255, 255, 255));
                SetDCBrushColor(dc, theme::color(255, 255, 255));
                GetStockObject(DC_BRUSH) as isize
            }
            WM_PAINT | WM_PRINTCLIENT => {
                AUX_PAINTS.fetch_add(1, Ordering::Relaxed);
                let mut ps: PAINTSTRUCT = zeroed();
                let dc = if msg == WM_PAINT {
                    BeginPaint(hwnd, &mut ps)
                } else {
                    wp as HDC
                };
                let mut r = RECT::default();
                GetClientRect(hwnd, &mut r);
                fill(dc, r, theme::color(249, 250, 253));
                SetBkMode(dc, TRANSPARENT as i32);
                let dpi = window_dpi(hwnd);
                theme::heading(
                    dc,
                    tr("AI conversation"),
                    RECT {
                        left: scale(24, dpi),
                        top: scale(18, dpi),
                        right: r.right - scale(24, dpi),
                        bottom: scale(54, dpi),
                    },
                    22,
                    theme::color(31, 44, 63),
                    true,
                    dpi,
                );
                if msg == WM_PAINT {
                    EndPaint(hwnd, &ps);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
