// Decision AH-D1: an initial procedure made only of concurrent assertions
// begins one evaluation attempt per assertion, at its first leading clock
// event, also when the assertion is labeled inside a begin-end block, nested
// in further blocks, or one of several assertions in the procedure (S38-D6
// covers an assertion that is the whole procedure).
//
// IEEE 1800-2009 16.15.6 (SystemVerilog-1800-2009.txt L26750-26753): "the
// assertion ... [is] placed in a procedural assertion queue associated with
// the currently executing process", and an instance that matures before its
// clocking event "will cause the assertion to begin an evaluation attempt upon
// the next clocking event" (L26758-26766). 16.3 (L21105-21106): "The optional
// statement label (identifier and colon) creates a named block around the
// assertion statement". An initial procedure executes once, so each assertion
// in it is queued once.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b1, b = 1'b0;
  initial begin : ib
    l1: assert property (@(posedge clk) a ##1 b)
      $display("%0t l1 pass", $time); else $display("%0t l1 fail", $time);
  end
  initial begin
    l2: cover property (@(posedge clk) a) $display("%0t l2 cover", $time);
    begin : inner
      l3: cover property (@(posedge clk) a ##2 !b) begin : act
        $display("%0t l3 cover", $time);
      end
    end
  end
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    b = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    b = 1'b0;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
endmodule
