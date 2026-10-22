// Decision S27-D7: a word whose address is outside the memory is an error
// that terminates the load and keeps the words already loaded, whether the
// address comes from an `@` specification in the file or from counting on
// from the start address. A start or finish argument outside the memory is
// not rejected by itself: the load stores the words whose addresses are
// inside the memory and ends at the first word that is not.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37280,
// L37315-37317):
//   "As the file is read, each number encountered is assigned to a successive
//   word element of the memory."
//   "... the addresses in the data file shall be within the address range
//   specified by the system task arguments; otherwise, an error message is
//   issued, and the load operation is terminated."
//
// The text states the error only for a range given in the task. llg follows
// industry-standard behaviour: the memory's own bounds end the load when a
// word reaches them, and task arguments outside the memory are checked word
// by word rather than up front.
module tb;
  logic [7:0] mem [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d7.mem", "w");
    $fwrite(fd, "@1\n11\n@9\n22\n");
    $fclose(fd);
    fd = $fopen("s27_d7_plain.mem", "w");
    $fwrite(fd, "aa\nbb\ncc\n");
    $fclose(fd);
    mem = '{default: 8'hee};
    $readmemh("s27_d7.mem", mem);
    $display("file address %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    mem = '{default: 8'hee};
    $readmemh("s27_d7_plain.mem", mem, 2, 5);
    $display("finish outside %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    mem = '{default: 8'hee};
    $readmemh("s27_d7_plain.mem", mem, 5);
    $display("start outside %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule
