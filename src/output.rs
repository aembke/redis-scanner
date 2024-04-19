use crate::argv::OutputFormat;

///
pub trait Output {
  fn format(&self) -> OutputFormat;
  ///
  fn print_table(self: Box<Self>) -> String;

  ///
  fn print_json(self: Box<Self>) -> String;

  ///
  fn print_csv(self: Box<Self>) -> String;

  fn print(self: Box<Self>) -> String {
    match self.format() {
      OutputFormat::Table => self.print_table(),
      OutputFormat::Csv => self.print_csv(),
      OutputFormat::Json => self.print_json(),
    }
  }
}
