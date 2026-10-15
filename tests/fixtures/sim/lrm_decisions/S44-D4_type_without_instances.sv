// Decision S44-D4: get_coverage() of a covergroup type that has no instance
// returns 0.0, and the type does not contribute to $get_coverage().
//
// IEEE 1800-2009 19.11 (SystemVerilog-1800-2009.txt L33808-33811): "Any zero
// denominator in the coverage calculation shall result in the following: a)
// The covergroup does not contribute to the overall coverage score. b) If the
// covergroup's weight is non-zero, a value of 0.0 is returned by
// get_coverage and get_inst_coverage." L33817-33818: "$get_coverage shall
// return a value of 100.0 if called on a design that has no covergroup
// instances". The text lists the zero-denominator cases of one group; llg
// reads a type without instances (an empty instance sum, 19.11.3) as one.
module tb;
  bit [1:0] a;
  covergroup never_built;
    coverpoint a;
  endgroup
  covergroup built;
    coverpoint a;
  endgroup
  built b;
  initial begin
    $display("empty=%0.2f total=%0.2f", never_built::get_coverage(), $get_coverage());
    b = new;
    a = 1;
    b.sample();
    a = 2;
    b.sample();
    $display("empty=%0.2f built=%0.2f total=%0.2f", never_built::get_coverage(),
             built::get_coverage(), $get_coverage());
    $finish;
  end
endmodule
