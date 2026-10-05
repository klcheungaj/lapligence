// RTL-106 nearest-legal composition: IEEE 1364-2001 forms, several built by
// macros, that the strict 2001 profile admits and executes.
`timescale 1ns/1ns
`define WIDTH 8
`define ADD(x, y) ((x) + (y))
`define NAMED(name) begin : name
`define SHOW_CALL(v) show(v)

primitive inv_p(out, in);
  output out;
  input in;
  table
    0 : 1;
    1 : 0;
  endtable
endprimitive

module child #(parameter W = 4, parameter [3:0] INIT = 0)
              (input wire [W-1:0] a, output wire [W-1:0] y, output reg [W-1:0] r);
  assign y = a ^ INIT;
  always @* r = a + 1;
endmodule

module tb;
  reg [`WIDTH-1:0] a, b;
  wire [`WIDTH-1:0] sum;
  wire [3:0] y4, r4, y2, r2;
  wire nb, w_and;
  wire [3:0] gx;
  integer i;
  real rv;
  time t;
  reg [7:0] mem [0:3];
  reg [3:0] grid [0:1][0:1];
  (* keep *) reg p, q;
  reg [3:0] hits;
  event ev;
  genvar g;

  assign sum = `ADD(a, b);
  child #(.W(4), .INIT(4'h5)) u_child(.a(a[3:0]), .y(y4), .r(r4));
  child u2(a[3:0], y2, r2);
  defparam u2.INIT = 4'h1;
  inv_p u_inv(nb, a[0]);
  and g_and(w_and, a[0], b[0]);

  generate
    for (g = 0; g < 4; g = g + 1) begin : bits
      assign gx[g] = a[g] & b[g];
    end
  endgenerate

  function [7:0] twice;
    input [7:0] v;
    twice = v << 1;
  endfunction

  function automatic integer fact(input integer n);
    if (n <= 1) fact = 1; else fact = n * fact(n - 1);
  endfunction

  task show;
    input [7:0] v;
    $display("show %0d", v);
  endtask

  always @(posedge p or negedge q) hits = hits + 1;
  always @(ev) $display("event at %0t", $time);

  initial begin : main
    hits = 0;
    p = 0;
    q = 1;
    a = 8'd3;
    b = 8'd4;
    #1;
    $display("sum=%0d y4=%h r4=%h y2=%h r2=%h nb=%b and=%b gx=%b", sum, y4, r4, y2, r2, nb, w_and, gx);
    mem[0] = twice(8'd5);
    mem[1] = mem[0] ** 2;
    grid[1][0] = 4'b1010;
    $display("mem=%0d %0d grid=%b part=%b", mem[0], mem[1], grid[1][0], a[0 +: 2]);
    i = fact(5);
    rv = 1.5 * 2;
    $display("fact=%0d real=%f signed=%0d shift=%0d", i, rv, $signed(4'b1100), -8 >>> 1);
    `SHOW_CALL(`ADD(8'd1, 8'd2));
    fork
      #2 $display("fork a at %0t", $time);
      #1 $display("fork b at %0t", $time);
    join
    -> ev;
    #1;
    t = $time;
    case (a)
      8'd3: $display("case three t=%0d", t);
      default: $display("case other");
    endcase
    casez (4'b1z01)
      4'b1?01: $display("casez hit");
      default: $display("casez miss");
    endcase
    repeat (2) b = b + 1;
    $display("b=%0d", b);
    i = 0;
    `NAMED(loop_blk)
      while (1) begin
        i = i + 1;
        if (i == 3) disable loop_blk;
      end
    end
    wait (i == 3) $display("while i=%0d", i);
    force sum = 8'd99;
    #1 $display("forced=%0d", sum);
    release sum;
    #1 $display("released=%0d", sum);
    a <= #1 8'd10;
    #2 $display("nba a=%0d", a);
    p = 1;
    #1 q = 0;
    #1 $display("hits=%0d at %0t", hits, $time);
    $finish;
  end
endmodule
