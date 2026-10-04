use crate::token::{keyword, Token, TokenType};

/// Cut `source` into tokens. Returns everything it managed to scan alongside every
/// error it found; the caller decides whether to go on.
pub fn scan(source: &str) -> (Vec<Token>, Vec<String>) {
    let mut s = Scanner {
        src: source.chars().collect(),
        start: 0,
        current: 0,
        line: 1,
        tokens: Vec::new(),
        errors: Vec::new(),
    };
    s.run();
    (s.tokens, s.errors)
}

struct Scanner {
    src: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
    tokens: Vec<Token>,
    errors: Vec<String>,
}

impl Scanner {
    fn run(&mut self) {
        while !self.at_end() {
            self.start = self.current;
            self.scan_token();
        }
        self.start = self.current;
        let eof_line = match self.tokens.last() {
            Some(t) => t.line,
            None => 1,
        };
        self.line = eof_line;
        self.add(TokenType::Eof);
    }

    fn scan_token(&mut self) {
        let c = self.advance();
    match c {
    '(' => self.add(TokenType::LParen),
    ')' => self.add(TokenType::RParen),
    '{' => self.add(TokenType::LBrace),
    '}' => self.add(TokenType::RBrace),
    ',' => self.add(TokenType::Comma),
    '-' => self.add(TokenType::Minus),
    '+' => self.add(TokenType::Plus),
    ';' => self.add(TokenType::Semicolon),
    '*' => self.add(TokenType::Star),
    '!' => {
        let t = if self.matches('=') { TokenType::BangEqual } else { TokenType::Bang };
        self.add(t);
    }
    '=' => {
        let t = if self.matches('=') { TokenType::EqualEqual } else { TokenType::Equal };
        self.add(t);
    }
    '<' => {
        let t = if self.matches('=') { TokenType::LessEqual } else { TokenType::Less };
        self.add(t);
    }
    '>' => {
        let t = if self.matches('=') { TokenType::GreaterEqual } else { TokenType::Greater };
        self.add(t);
    }
    '/' => {
        if self.matches('/') {
            while self.peek() != '\n' && !self.at_end() {
                self.advance();
            }
        } else {
            self.add(TokenType::Slash);
        }
    }
    ' ' | '\r' | '\t' => {}
    '\n' => self.line += 1,
    '"' => self.string(),
    c if c.is_ascii_digit() => self.number(),
    c if c.is_ascii_alphabetic() || c == '_' => self.identifier(),
    _ => {
        let line = self.line;
        self.error(line, "Character is not part of any token.");
    }
    }
    }


    fn string(&mut self) {
        let opened = self.line;
        while !self.at_end() && self.peek() != '"' {
            if self.peek() == '\n' {
                self.line +=1;
            }
            self.advance();
        }
        if self.at_end() {
            self.error(opened, "String is never closed.");
            return;
        }
        self.advance();
        self.add(TokenType::Str);
        // TODO(you): scan a string literal. A string may span lines (1.5); an unterminated one
        //            is reported at the line it opened on (5.1).
    }

    fn number(&mut self) {
        while self.peek().is_ascii_digit() {
            self.advance();
        }
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }
        self.add(TokenType::Number);
        // TODO(you): scan a number literal: digits, then a fractional part only when a digit
        //            follows the dot (1.4).
    }

    fn identifier(&mut self) {
        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
            self.advance();
        }
       let word: String = self.src[self.start..self.current].iter().collect();
        self.add(keyword(&word).unwrap_or(TokenType::Identifier));
        // TODO(you): scan an identifier, then decide whether it is a keyword; keyword() in
        //            token.rs does the lookup (1.2, 1.3).
    }

    // --- primitives ---------------------------------------------------------------

    fn at_end(&self) -> bool {
        self.current >= self.src.len()
    }

    fn advance(&mut self) -> char {
        let c = self.src[self.current];
        self.current += 1;
        c
    }

    fn matches(&mut self, expected: char) -> bool {
        if self.at_end() || self.src[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn peek(&self) -> char {
        if self.at_end() {
            '\0'
        } else {
            self.src[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.src.len() {
            '\0'
        } else {
            self.src[self.current + 1]
        }
    }

    fn add(&mut self, kind: TokenType) {
        self.tokens.push(Token {
            kind,
            lexeme: self.src[self.start..self.current].iter().collect(),
            line: self.line,
        });
    }

    fn error(&mut self, line: usize, message: &str) {
        self.errors.push(format!("[line {}] Error: {}", line, message));
    }
}
