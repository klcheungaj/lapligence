// Decision S45-D6: in a bins array `b[]`, a value listed more than once gets
// one bin; the array has one bin per distinct value.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32413-32415): "The bins
// construct allows creating a separate bin for each value in the given range
// list or a single bin for the entire range of values. To create a separate
// bin for each value (an array of bins), the square brackets, [], shall
// follow the bin name." The text does not say whether a duplicated value
// creates a second bin; llg reads "each value" as each distinct value (a
// bin is named by its value, so two bins of one value would share a name).
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a { bins each[] = {1, [3:4], 3}; }
  endgroup
  g c = new;
  initial begin
    a = 4'd3;
    c.sample();
    $display("after 3: %0.2f", c.get_inst_coverage());
    a = 4'd1;
    c.sample();
    $display("after 1: %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
