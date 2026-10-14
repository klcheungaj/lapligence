// Decision S27-D8: when a file without task addresses reaches the highest
// address and still has words, the words that have no cell are skipped and
// a later `@` address resumes loading.
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
// The text does not say whether reaching the highest address ends a file
// that has a later address specification; llg skips the words that have no
// cell, warns, and honours the later address.
module tb;
  logic [7:0] mem [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d8.mem", "w");
    $fwrite(fd, "@2\n11\n22\n33\n44\n@0\naa\n");
    $fclose(fd);
    mem = '{default: 8'hee};
    $readmemh("s27_d8.mem", mem);
    $display("mem %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule
