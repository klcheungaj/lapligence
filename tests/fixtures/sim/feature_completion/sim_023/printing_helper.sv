`timescale 1ns/1ns
// SIM-023: forces whose right-hand side calls a function that prints. The
// lines that start with `P` are llg's: how often a source is evaluated is
// unspecified. Every other line is what any simulator prints.
module tb;
  int x, y, count;
  real rx;
  wire [3:0] w;
  logic [3:0] d = 4'h1;
  assign w = d;

  function int show(int value);
    $display("P show %0d", value);
    return value;
  endfunction
  function int noted(int value);
    $info("P noted %0d", value);
    $write("P written %0d\n", value);
    $fdisplay(1, "P filed %0d", value);
    return value + 1;
  endfunction
  function int outer(int value);
    return show(value) * 2;
  endfunction
  function automatic int counted(int value);
    count++;
    $display("P counted");
    return value;
  endfunction

  initial begin
    y = 5;
    force x = show(2);
    $display("constant %0d", x);
    #1 x = 9;
    #1 $display("held %0d", x);
    release x;
    x = 3;
    $display("released %0d", x);
    force x = show(y);
    $display("variable %0d", x);
    #1 y = 6;
    #1 $display("followed %0d", x);
    force x = noted(y) + outer(y);
    $display("replaced %0d", x);
    #1 y = 7;
    #1 $display("followed %0d", x);
    release x;
    #1 y = 8;
    #1 $display("kept %0d", x);
    force w = show(y);
    #1 d = 4'h2;
    #1 $display("net %h", w);
    release w;
    $display("net released %h", w);
    force rx = show(y);
    #1 $display("real %0.1f", rx);
    #1 y = 4;
    #1 $display("real %0.1f", rx);
    release rx;
    force x = counted(y);
    $display("counted %0d %0d", x, count > 0);
    $finish;
  end
endmodule
