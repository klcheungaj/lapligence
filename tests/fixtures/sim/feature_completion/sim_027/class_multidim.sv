// SIM-027 A03: multidimensional fixed-array class properties as memory_name
// (IEEE 1800-2009 21.4, 21.4.3, 8.4): the whole property, a row selected by a
// constant or runtime index, a variable slice of a row, through a handle,
// through `this.` and a static property; loads and dumps read back. The
// property is also copied whole, compared, printed with `%p` and assigned a
// nested positional pattern. A row index outside the declared range changes
// nothing (S27-D19).
module tb;
  class holder;
    logic [7:0] mem [1:2][3:0];
    static byte table_ [0:1][0:1];
    function void load(int i);
      $readmemh("words.mem", this.mem[i], 0, 3);
      $readmemh("words.mem", table_);
    endfunction
  endclass

  holder h;
  holder other;
  logic [7:0] copy [1:2][3:0];
  logic [7:0] back [0:3];
  int i;

  initial begin
    int fd;
    fd = $fopen("words.mem", "w");
    $fwrite(fd, "11\n22\n33\n44\n55\n66\n77\n88\n");
    $fclose(fd);
    h = new;
    other = new;
    // The whole property: rows low to high, each row low to high.
    $readmemh("words.mem", h.mem);
    $display("whole %p", h.mem);
    // A variable slice of a runtime row.
    i = 2;
    $readmemh("words.mem", h.mem[i][i-:2]);
    $display("slice %p", h.mem);
    // `this.` with an explicit range, and a static property.
    h.load(1);
    $display("this %p static %p", h.mem, holder::table_);
    // A dump of a row, read back.
    $writememh("dump.mem", h.mem[2]);
    $readmemh("dump.mem", back);
    $display("dump %h %h %h %h", back[0], back[1], back[2], back[3]);
    // Whole copies, equality and a nested positional pattern.
    copy = h.mem;
    other.mem = h.mem;
    $display("copy %p equal %0d", copy, h.mem == other.mem);
    other.mem = '{'{8'h01, 8'h02, 8'h03, 8'h04}, '{8'h05, 8'h06, 8'h07, 8'h08}};
    $writememh("dump.mem", other.mem[1]);
    $readmemh("dump.mem", back);
    $display("pattern %p dump %h %h %h %h", other, back[0], back[1], back[2], back[3]);
    // A row outside the declared range: nothing changes.
    i = 3;
    $readmemh("words.mem", h.mem[i]);
    $display("kept %p", h.mem);
    $finish;
  end
endmodule
