// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::sync::atomic::{AtomicBool, Ordering};

rust_i18n::i18n!("locales", fallback = "en");

static LANG_SET: AtomicBool = AtomicBool::new(false);

pub fn set_language(lang: &str) {
    rust_i18n::set_locale(lang);
    LANG_SET.store(true, Ordering::Relaxed);
}

pub fn current_language() -> String {
    if LANG_SET.load(Ordering::Relaxed) {
        rust_i18n::locale().to_string()
    } else {
        std::env::var("LANG")
            .unwrap_or_default()
            .split('.')
            .next()
            .unwrap_or("en")
            .split('_')
            .next()
            .unwrap_or("en")
            .to_string()
    }
}
