// Lifetime of an effectful force source: re-execution, release, re-force and
// a force issued by a static task (SV 10.6.2, 13.3.2).
module tb;
  logic [7:0] x = 8'd1, y, t;
  int cnt = 0;

  function automatic logic [7:0] bump(input logic [7:0] v);
    cnt++;
    return v + 8'd1;
  endfunction

  task tforce(input logic [7:0] k);
    force t = bump(k + x);
  endtask

  initial begin
    repeat (2) force y = bump(x);
    #1 $display("a y=%0d", y);
    release y;
    cnt = 0;
    x = 8'd4;
    #1 $display("b y=%0d cnt=%0d", y, cnt);
    force y = bump(x);
    #1 x = 8'd8;
    #1 $display("c y=%0d", y);
    tforce(8'd10);
    #1 $display("d t=%0d", t);
    x = 8'd2;
    #1 $display("e t=%0d", t);
    tforce(8'd20);
    #1 $display("f t=%0d", t);
    release t;
    release y;
    cnt = 0;
    x = 8'd3;
    #1 $display("g y=%0d t=%0d cnt=%0d", y, t, cnt);
    $finish(0);
  end
endmodule
