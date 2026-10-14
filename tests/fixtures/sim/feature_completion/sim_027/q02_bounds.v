// SIM-027 Q02 resolution of range intersections (IEEE 1364-2001 17.2.8;
// IEEE 1800-2009 21.4): with start and finish given, a file address outside
// that range is an error that terminates the load, keeping the words already
// placed; a file with too few words leaves the remaining cells unchanged; a
// file with too many words fills the range and stops there. The count cases
// are warnings. Expected values are derived by hand.
module tb;
  reg [7:0] mem [0:3];
  integer i;
  initial begin
    for (i = 0; i < 4; i = i + 1) mem[i] = 8'hff;
    $readmemh("far_address.hex", mem, 0, 2);
    $display("far_address %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    for (i = 0; i < 4; i = i + 1) mem[i] = 8'hff;
    $readmemh("two_words.hex", mem, 0, 3);
    $display("short %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    for (i = 0; i < 4; i = i + 1) mem[i] = 8'hff;
    $readmemh("three_words.hex", mem, 3, 2);
    $display("long %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule
