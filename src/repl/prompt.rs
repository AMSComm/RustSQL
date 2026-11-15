use colored::Colorize;

pub struct PromptBuilder {
    pub dbname: String,
    pub is_superuser: bool,
}

impl PromptBuilder {
    pub fn new(dbname: String, is_superuser: bool) -> Self {
        Self {
            dbname,
            is_superuser,
        }
    }

    pub fn primary_prompt(&self) -> String {
        let sym = if self.is_superuser { "#" } else { "=>" };
        format!("{}{} ", self.dbname.bold(), sym.green())
    }

    pub fn continuation_prompt(&self) -> String {
        format!("{}{} ", self.dbname.bold(), "->".yellow())
    }
}
