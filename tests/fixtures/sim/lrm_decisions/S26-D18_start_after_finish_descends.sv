// Decision S26-D18 (llg reading): $sreadmemb/$sreadmemh load their words from
// start_address toward finish_address, descending when start_address is the
// larger, exactly as $readmem* does; words past the finish address are not
// loaded and cells outside the range keep their values.
//
// IEEE 1800-2009 D.14 (SystemVerilog-1800-2009.txt L70158-70160): "The start
// and finish addresses indicate the bounds for where the data from strings will
// be stored in the memory. These strings take the same format as the strings
// that appear in the input files". D.14 does not state the direction.
// 21.4 (L37310-37313) does for $readmem*: "If both start and finish addresses
// are specified as arguments to the task, then loading shall begin at the start
// address and shall continue toward the finish address. If the start address is
// greater than the finish address, then the address will be decremented between
// consecutive loads rather than being incremented."
module tb;
  logic [7:0] m [0:7];
  initial begin
    m = '{default: 8'h5a};
    $sreadmemh(m, 5, 2, "a b", "c d e");
    $display("m %h %h %h %h %h %h %h %h", m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7]);
    $finish;
  end
endmodule
