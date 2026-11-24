// FIN-002 step 3d regression (expression corpus, minimized): an unbased
// unsized literal below an operator inside a size cast takes the size of the
// cast, like any operand of an assignment to a vector of that size.
//
// IEEE 1800-2009 6.24.1: "When changing the size, the cast shall return the
// value that a packed array type with a single [n-1:0] dimension would hold
// after being assigned the expression". 5.7.1: "In a self-determined
// context, an unsized single-bit value shall have a width of 1 bit"; in an
// assignment it has the size of the context (11.6.1). The operators here
// take their size from their operands: unary operators, the conditional
// operator and the left operand of a shift or a power.
module tb;
  localparam logic [31:0] P_NOT = 32'(~'0);
  localparam logic [31:0] P_SHR = 32'('1 >> 1'b1);
  localparam logic [7:0] P_SHL = 8'('1 << 3);
  localparam logic [7:0] P_COND = 8'(1'b1 ? '1 : '0);
  logic [31:0] t;
  logic [7:0] r;
  logic [3:0] a;
  logic [7:0] b;
  logic one;
  function automatic logic [7:0] f(input logic [7:0] v);
    return v;
  endfunction
  initial begin
    a = 4'd3;
    b = 8'hf0;
    one = 1'b1;
    #1;
    $display("constant %h %h %h %h", P_NOT, P_SHR, P_SHL, P_COND);
    t = 32'(~'0);            $display("not    %h", t);
    t = 32'('1 >> one);      $display("shr    %h", t);
    t = 32'('x >> 1'b1);     $display("shr x  %b", t);
    r = 8'('1 << a);         $display("shl    %h", r);
    t = 8'(one ? '1 : '0);   $display("cond   %h", t);
    t = 16'('1 >> 1'b1);     $display("narrow %h", t);
    t = 32'('x ** '0);       $display("power  %b", t);
    t = 8'(-'1);             $display("minus  %h", t);
    t = 8'(one + ~'0);       $display("sum    %h", t);
    t = 8'(~'x | 8'd0);      $display("or     %b", t);
    $display("display %b", 12'('1 >> 1'b1));
    $display("sized   %b %b", 12'(b << 2), 12'(-b));
    $display("actual  %b", f('x ** a));
    r = '1 >> a;             $display("assign %h", r);
    r = (a ? '1 : '0) ** 2;  $display("base   %h", r);
    $finish;
  end
endmodule
