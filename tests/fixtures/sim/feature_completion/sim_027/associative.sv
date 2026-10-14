// SIM-027 A03: associative arrays with integral index types as memory_name
// (IEEE 1800-2009 21.4.1, 21.5.3; decisions S27-D15..D17). A load creates or
// updates the entry at each address and leaves the others; addresses are
// cast to the index type; a dump writes the existing entries in index order,
// each preceded by its `@` address.
module tb;
  typedef enum logic [3:0] { RED = 1, GREEN = 4, BLUE = 9 } color_t;
  class holder;
    logic [7:0] table_ [shortint];
    task load();
      $readmemh("words.mem", table_, -2);
      foreach (table_[k]) $display("property[%0d]=%h", k, table_[k]);
    endtask
  endclass
  logic [7:0] by_int [int];
  bit [7:0] by_byte [byte unsigned];
  logic [11:0] by_color [color_t];
  logic [7:0] by_wide [bit [99:0]];
  logic [7:0] empty [int];
  logic [7:0] back [0:10];
  holder h;
  int fd;

  task automatic local_binary;
    logic [3:0] nibbles [longint unsigned];
    $readmemb("bits.mem", nibbles, 3, 5);
    foreach (nibbles[k]) $display("local[%0d]=%b", k, nibbles[k]);
    $writememb("local.mem", nibbles, 4);
  endtask

  task automatic show_file(string name);
    string line;
    int f;
    f = $fopen(name, "r");
    while ($fgets(line, f) != 0) $write("  %s", line);
    $fclose(f);
  endtask

  initial begin
    fd = $fopen("words.mem", "w");
    $fwrite(fd, "11 22\n@a\n33\n@ffffffff\n44 55\n");
    $fclose(fd);
    fd = $fopen("bits.mem", "w");
    $fwrite(fd, "1x01 0z10\n");
    $fclose(fd);
    by_int[1] = 8'h99;
    by_int[100] = 8'h77;
    $readmemh("words.mem", by_int);
    foreach (by_int[k]) $display("int[%0d]=%h", k, by_int[k]);
    $writememh("int.mem", by_int);
    $display("int.mem:");
    show_file("int.mem");
    $readmemh("words.mem", by_byte, 8'hfe);
    foreach (by_byte[k]) $display("byte[%0d]=%h", k, by_byte[k]);
    $writememh("byte.mem", by_byte, 255, 0);
    $display("byte.mem 255..0:");
    show_file("byte.mem");
    $readmemh("words.mem", by_color, GREEN, BLUE);
    foreach (by_color[k]) $display("color[%0d]=%h", k, by_color[k]);
    by_wide[100'h1_0000_0000_0000_0000] = 8'h05;
    $writememh("wide.mem", by_wide);
    $display("wide.mem:");
    show_file("wide.mem");
    local_binary();
    $display("local.mem from 4:");
    show_file("local.mem");
    h = new;
    h.load();
    $writememh("empty.mem", empty);
    $display("empty.mem:");
    show_file("empty.mem");
    // A dump read back into a fixed memory places each word at its address.
    $writememh("positive.mem", by_int, 0, 10);
    $readmemh("positive.mem", back);
    $display("back %h %h %h %h", back[0], back[1], back[2], back[10]);
    $finish;
  end
endmodule
