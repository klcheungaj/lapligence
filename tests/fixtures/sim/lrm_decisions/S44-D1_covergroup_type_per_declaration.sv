// Decision S44-D1: a covergroup declared in a module is one covergroup type
// for every instance of that module; get_coverage() of any instance averages
// the instances of all module instances.
//
// IEEE 1800-2009 19.8 (SystemVerilog-1800-2009.txt L33569-33571): "The
// get_coverage() method returns the cumulative (or type) coverage, which
// considers the contribution of all instances of a particular coverage item;
// and it is a static method that is available on both types (via the ::
// operator) and instances (using the . operator)." 19.11.3 (L33937-33938):
// "When type_option.merge_instances is false, type coverage is computed as
// the weighted average of all instances." The text does not say whether the
// covergroup declarations elaborated in two module instances are one type:
// llg treats one source declaration as one type.
module unit(input logic clk, input logic [1:0] d);
  covergroup cg @(posedge clk);
    coverpoint d;
  endgroup
  cg c = new;
endmodule

module tb;
  logic clk = 0;
  logic [1:0] d0, d1;
  unit u0(clk, d0);
  unit u1(clk, d1);
  initial begin
    d0 = 0;
    d1 = 1;
    #1 clk = 1;
    #1 clk = 0;
    d0 = 2;
    #1 clk = 1;
    #1 clk = 0;
    $display("u0=%0.2f u1=%0.2f", u0.c.get_inst_coverage(), u1.c.get_inst_coverage());
    $display("type via u0=%0.2f via u1=%0.2f", u0.c.get_coverage(), u1.c.get_coverage());
    $finish;
  end
endmodule
