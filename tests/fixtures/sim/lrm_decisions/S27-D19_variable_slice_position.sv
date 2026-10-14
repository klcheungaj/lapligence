// Decision S27-D19: a slice with a variable position (`[i +: w]`,
// `[i -: w]`) is a valid memory_name. Its position is evaluated once when the
// task runs, after any higher-dimension index and before the start and
// finish addresses, and the task addresses the selected indices as it would
// a constant slice in the same place. A slice that is not wholly inside the
// array (its declared range, or the current size of a dynamic array), or
// whose position has an X or Z bit, is an error: nothing is loaded, and a
// dump writes no file, so an existing file keeps its contents.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8226):
//   "The size of the part-select or slice shall be constant, but the position
//   can be variable."
// IEEE 1800-2009 7.4.6 (L8235-8237):
//   "If an index expression is out of the address bounds or if any bit in the
//   address is X or Z, then the index shall be invalid. ... Writing to an
//   array with an invalid index shall perform no operation."
// IEEE 1800-2009 21.4 (L37253-37254):
//   "The lowest dimension (i.e., the rightmost specified dimension in the
//   identifier) can be specified with slice syntax."
//
// The text does not say when a variable slice position is evaluated or what a
// memory task does with an invalid slice; llg evaluates it once, like the
// memory_name (S27-D13), and rejects the call, as it rejects a slice outside
// the current size (S27-D14) and a range outside the array (S27-D7, S27-D12).
module tb;
  logic [7:0] up [0:7];
  logic [7:0] down [7:0];
  logic [7:0] dyn [];
  logic [7:0] back [0:1];
  integer i;
  integer fd;
  integer calls;

  function automatic integer at(integer value);
    calls = calls + 1;
    return value;
  endfunction

  initial begin
    fd = $fopen("s27_d19.mem", "w");
    $fwrite(fd, "11\n22\n");
    $fclose(fd);
    calls = 0;
    i = 2;
    $readmemh("s27_d19.mem", up[at(i)+:2]);
    $display("up %h %h %h %h calls %0d", up[1], up[2], up[3], up[4], calls);
    $readmemh("s27_d19.mem", down[i+3-:2]);
    $display("down %h %h %h %h", down[3], down[4], down[5], down[6]);
    dyn = new[4];
    $readmemh("s27_d19.mem", dyn[i-:2], 2, 1);
    $display("dyn %h %h %h %h size %0d", dyn[0], dyn[1], dyn[2], dyn[3], dyn.size());
    $writememh("s27_d19_dump.mem", up[i+:2]);
    // Not inside the array, or an unknown position: nothing happens.
    i = 7;
    $readmemh("s27_d19.mem", up[i+:2]);
    $readmemh("s27_d19.mem", dyn[i-:2]);
    i = 'x;
    $readmemh("s27_d19.mem", up[i+:2]);
    $writememh("s27_d19_dump.mem", down[i-:2]);
    $display("after up %h %h dyn %h %h", up[7], up[0], dyn[2], dyn[3]);
    $readmemh("s27_d19_dump.mem", back);
    $display("dump %h %h", back[0], back[1]);
    $finish;
  end
endmodule
