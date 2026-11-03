// FIN-001 G3 (F06-04): blocking timed writes of a bit, part or indexed part
// select inside one packed element of a queue, dynamic or associative array.
// The value is captured before the timing control; the element, its index or
// key and its other bits are those current when the control completes
// (SV 9.4.5, 10.4.1).
`timescale 1ns / 1ns
module tb;
  event e;
  int i;
  string key;
  logic [7:0] q[$];
  logic [7:0] d[];
  logic [7:0] ai[int];
  logic [15:0] as[string];
  logic [3:0] v;

  // `e` occurs at 4, 8, 12, ...
  initial forever #4 ->e;

  initial begin
    q = '{8'h00, 8'h00, 8'h00};
    d = new[2];
    ai[3] = 8'h00;
    ai[5] = 8'h00;
    as["a"] = 16'h0000;
    as["b"] = 16'h0000;
    i = 0;
    v = 4'hA;
    fork
      q[i][3:0] = @e v;
      #1 begin
        i = 2;
        v = 4'h1;
        q[2] = 8'hF0;
      end
    join
    #1 $display("%0t q: %h %h %h", $time, q[0], q[1], q[2]);
    i = 0;
    fork
      d[i][i+4 +: 2] = repeat (2) @e 2'b11;
      #1 begin
        i = 1;
        d[1] = 8'h01;
      end
    join
    #1 $display("%0t d: %h %h", $time, d[0], d[1]);
    i = 3;
    fork
      ai[i][7] = #2 1'b1;
      #1 i = 5;
    join
    #1 $display("%0t ai: %h %h", $time, ai[3], ai[5]);
    key = "a";
    fork
      as[key][15:8] = @e 8'h5A;
      #1 begin
        key = "b";
        as["b"] = 16'h00C3;
      end
    join
    #1 $display("%0t as: %h %h", $time, as["a"], as["b"]);
    // A key absent when the control completes creates the element from the
    // default value (SV 7.8.6) and then writes the selected bits.
    key = "c";
    as[key][3:0] = #1 4'h9;
    $display("%0t as[c]: %h %0d", $time, as["c"], as.num());
    $finish;
  end
endmodule
