// Decision S27-D8: a file without task addresses that reaches the highest
// address and still has words is an error: the next word has no cell, so
// the load terminates there and keeps the words already loaded; a later `@`
// address is not reached. Words past the finish address of a task range
// that still have cells in the memory are skipped with a warning, and a
// later `@` address inside the range resumes loading.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37303-37308, L37300-37301):
//   "If no addressing information is specified within the system task and no
//   address specifications appear within the data file, then the default
//   start address shall be the lowest address in the memory. Consecutive
//   words shall be loaded until either the highest address in the memory is
//   reached or the data file is completely read. ... In both cases, loading
//   shall continue upward even after an address specification in the data
//   file."
//   "When the system task encounters an address specification, it loads
//   subsequent data starting at that memory address."
//
// The text does not say what happens to words after the highest address.
// llg follows industry-standard behaviour: a word past the memory ends the
// load (as S27-D7), and a word past the range but inside the memory does not.
module tb;
  logic [7:0] mem [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d8.mem", "w");
    $fwrite(fd, "@2\n11\n22\n33\n44\n@0\naa\n");
    $fclose(fd);
    fd = $fopen("s27_d8_range.mem", "w");
    $fwrite(fd, "aa\nbb\ncc\ndd\n@1\nff\n");
    $fclose(fd);
    mem = '{default: 8'hee};
    $readmemh("s27_d8.mem", mem);
    $display("memory end %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    mem = '{default: 8'hee};
    $readmemh("s27_d8_range.mem", mem, 0, 2);
    $display("range end %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule
