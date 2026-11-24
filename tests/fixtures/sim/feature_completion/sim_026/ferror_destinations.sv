// $ferror destinations (21.3.7): integral variables and selections as well as
// strings. The text is right-justified; a narrow destination keeps its end.
`timescale 1ns/1ps
module tb;
  integer fd, e, n;
  string s;
  reg [639:0] wide;
  reg [31:0] narrow;
  reg [8*3:1] odd;
  bit [99:0] two_state;
  logic [0:79] ascending;
  struct packed { logic [15:0] a; logic [63:0] b; } record;
  reg [63:0] mem [0:1];
  reg [63:0] q [$];
  class C; logic [63:0] p; string t; endclass
  C c;

  task automatic show(output logic [63:0] text, input integer f);
    logic [63:0] local_text;
    e = $ferror(f, local_text);
    text = local_text;
  endtask

  function automatic integer status(integer f, ref logic [63:0] text);
    return $ferror(f, text);
  endfunction

  logic [63:0] r;
  always @(r) $display("r changed <%0s>", r);

  initial begin
    fd = $fopen("no_such_directory/none", "r");
    e = $ferror(fd, wide); $display("%0d <%0s>", e, wide);
    e = $ferror(fd, s); $display("%0d <%s> %0d", e, s, s.len());
    e = $ferror(fd, narrow); $display("%0d <%s> %h", e, narrow, narrow);
    e = $ferror(fd, odd); $display("%0d <%s> %h", e, odd, odd);
    e = $ferror(fd, two_state); $display("%0d <%s> %h", e, two_state, two_state);
    e = $ferror(fd, ascending); $display("%0d <%s> %h", e, ascending, ascending);
    e = $ferror(fd, record); $display("%0d <%s> %h", e, record, record.a);
    e = $ferror(fd, wide[31:0]); $display("%0d <%s>", e, wide[31:0]);
    e = $ferror(fd, n); $display("%0d %h", e, n);
    n = 1;
    e = $ferror(fd, mem[n]); $display("%0d <%s>", e, mem[1]);
    q.push_back(0);
    e = $ferror(fd, q[0]); $display("%0d <%s>", e, q[0]);
    c = new;
    e = $ferror(fd, c.p); $display("%0d <%s>", e, c.p);
    e = $ferror(fd, c.t); $display("%0d <%s>", e, c.t);
    show(narrow, fd); $display("%0d <%s>", e, narrow);
    e = status(fd, r); $display("%0d <%s>", e, r);
    #1;
    fd = $fopen("ferror_destinations.tmp", "w");
    wide = "zzz"; s = "zzz"; narrow = "zzzz";
    e = $ferror(fd, wide); $display("%0d <%0s>", e, wide);
    e = $ferror(fd, s); $display("%0d <%s>", e, s);
    e = $ferror(fd, narrow); $display("%0d %h", e, narrow);
    $fclose(fd);
    wide = "zzz"; s = "zzz";
    e = $ferror(fd, wide); $display("%0d <%0s>", e, wide);
    e = $ferror(fd, s); $display("%0d <%s>", e, s);
    fd = $fopen("ferror_destinations.tmp", "q");
    e = $ferror(fd, s); $display("%0d %0d", fd, e != 0);
    $finish;
  end
endmodule
