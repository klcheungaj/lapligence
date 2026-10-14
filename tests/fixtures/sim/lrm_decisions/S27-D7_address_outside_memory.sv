// Decision S27-D7: an address outside the memory is an error even when the
// task gives no range. A file address outside the memory terminates the
// load and keeps the words already loaded; a start or finish argument
// outside the memory rejects the whole call before any word is loaded.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37280,
// L37315-37317):
//   "As the file is read, each number encountered is assigned to a successive
//   word element of the memory."
//   "... the addresses in the data file shall be within the address range
//   specified by the system task arguments; otherwise, an error message is
//   issued, and the load operation is terminated."
//
// The text states the error only for a range given in the task; llg applies
// it to the memory's own bounds as well, since no word can be stored there.
module tb;
  logic [7:0] mem [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d7.mem", "w");
    $fwrite(fd, "@1\n11\n@9\n22\n");
    $fclose(fd);
    fd = $fopen("s27_d7_plain.mem", "w");
    $fwrite(fd, "aa\nbb\n");
    $fclose(fd);
    mem = '{default: 8'hee};
    $readmemh("s27_d7.mem", mem);
    $display("file address %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    mem = '{default: 8'hee};
    $readmemh("s27_d7_plain.mem", mem, 2, 5);
    $display("finish outside %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $readmemh("s27_d7_plain.mem", mem, -1);
    $display("start outside %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule
