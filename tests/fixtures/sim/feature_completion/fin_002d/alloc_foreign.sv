// Allocation failure matrix: DPI-C imports and exports with strings, open
// arrays, packed vectors and a context task, and a VPI application
// (alloc_foreign.c) that walks the design, reads and writes values in
// several formats and registers value-change, delay and end-of-run
// callbacks.
`timescale 1ns / 1ns
module tb;
  import "DPI-C" function int c_sum(input int values[]);
  import "DPI-C" function string c_join(input string left, input string right);
  import "DPI-C" function void c_wide(input logic [99:0] in, output logic [99:0] out);
  import "DPI-C" context task c_walk(input int steps);
  import "DPI-C" function chandle c_open(input string name);
  import "DPI-C" function int c_close(input chandle handle);
  export "DPI-C" function sv_note;
  export "DPI-C" task sv_wait;

  logic clk = 1'b0;
  logic [7:0] counter = '0;
  logic [99:0] wide = 100'h123456789abcdef0123456789;
  logic [99:0] echoed;
  logic [15:0] memory[4];
  string text = "vpi text";
  real level = 1.5;
  int values[];
  int notes;
  chandle session;

  function void sv_note(input string message, input int value);
    notes += value;
    $display("note %s %0d", message, value);
  endfunction

  task sv_wait(input int cycles);
    repeat (cycles) @(posedge clk);
  endtask

  always #5 clk = ~clk;
  always @(posedge clk) counter <= counter + 8'd1;

  initial begin
    foreach (memory[i]) memory[i] = 16'(i * 257);
    values = new[6];
    foreach (values[i]) values[i] = i * 11;
    $display("sum %0d", c_sum(values));
    $display("join %s", c_join("left", {text, "!"}));
    c_wide(wide, echoed);
    $display("wide %h", echoed);
    session = c_open("session");
    c_walk(3);
    $display("walk %0d at %0t close %0d", notes, $time, c_close(session));
    $inspect("tb");
    $watch("tb.counter");
    repeat (4) @(posedge clk);
    $poke("tb.wide", 77);
    $poke("tb.memory[2]", 4660);
    #1 $display("poked %0d %h", wide, memory[2]);
    $later(7);
    #20;
    $display("done");
    $finish;
  end
endmodule
