// Decision S27-D21: an element of a dynamic array or queue whose elements are
// unpacked arrays (`da[k]`) is a memory_name: a fixed unpacked array addressed
// in place. The element index is evaluated once when the task runs; the
// array is never resized. An element index outside the current size, or
// with an X or Z bit, is an error: nothing is loaded, and a dump writes no
// file, so an existing file keeps its contents. The whole array, or a slice
// of its elements, is not a memory_name (S27-D22).
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37251-37254):
//   "The memory_name can be an unpacked array or a partially indexed
//   multidimensional unpacked array that resolves to a lesser dimensioned
//   unpacked array (see 21.4.3). Higher order dimensions shall be specified
//   with an index, rather than a complete or partial dimension range."
// IEEE 1800-2009 21.4.1 (L37347):
//   "When loading dynamic arrays and queues, the current size of the array is
//   fixed; the array shall not be resized."
// IEEE 1800-2009 7.4.6 (L8235-8237):
//   "If an index expression is out of the address bounds or if any bit in the
//   address is X or Z, then the index shall be invalid. ... Writing to an
//   array with an invalid index shall perform no operation."
//
// The text does not say what a memory task does with an invalid element
// index; llg rejects the call, as it rejects a slice outside the array
// (S27-D14, S27-D19).
module tb;
  logic [7:0] da [][2];
  logic [7:0] back [0:1];
  integer fd;
  integer k;
  integer calls;

  function automatic integer at(integer value);
    calls = calls + 1;
    return value;
  endfunction

  // The element values are shown through a dump of each element read back
  // into a fixed array.
  logic [7:0] seen [0:2][0:1];
  task show(input string label);
    $writememh("s27_d21_e0.mem", da[0]);
    $writememh("s27_d21_e1.mem", da[1]);
    $writememh("s27_d21_e2.mem", da[2]);
    $readmemh("s27_d21_e0.mem", seen[0]);
    $readmemh("s27_d21_e1.mem", seen[1]);
    $readmemh("s27_d21_e2.mem", seen[2]);
    $display("%s %h %h %h %h %h %h size %0d", label, seen[0][0], seen[0][1], seen[1][0],
             seen[1][1], seen[2][0], seen[2][1], da.size());
  endtask

  initial begin
    fd = $fopen("s27_d21.mem", "w");
    $fwrite(fd, "11\n22\n");
    $fclose(fd);
    da = new[3];
    calls = 0;
    $readmemh("s27_d21.mem", da[at(1)]);
    show("row");
    $display("calls %0d", calls);
    $readmemh("s27_d21.mem", da[2]);
    show("last");
    $writememh("s27_d21_dump.mem", da[1]);
    k = 3;
    $readmemh("s27_d21.mem", da[k]);
    k = 'x;
    $readmemh("s27_d21.mem", da[k]);
    $writememh("s27_d21_dump.mem", da[k]);
    show("after");
    $readmemh("s27_d21_dump.mem", back);
    $display("dump %h %h", back[0], back[1]);
    $finish;
  end
endmodule
