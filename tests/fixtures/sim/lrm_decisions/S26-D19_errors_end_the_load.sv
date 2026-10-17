// Decision S26-D19 (llg choice): an address inside the strings that lies outside
// the start/finish range, and a token that is not a number, are errors that end
// the load; the words placed before the error stay and the simulation goes on.
//
// IEEE 1800-2009 D.14 (SystemVerilog-1800-2009.txt L70159-70160) says the
// strings "take the same format as the strings that appear in the input
// files" and is silent on errors. 21.4 (L37315-37317) for files: "the
// addresses in the data file shall be within the address range specified by
// the system task arguments; otherwise, an error message is issued, and the
// load operation is terminated." 21.4 (L37268-37271) lists what the text may
// contain: white space, comments and binary or hexadecimal numbers; it states
// no consequence for anything else.
module tb;
  logic [7:0] m [0:7];
  initial begin
    m = '{default: 8'h5a};
    $sreadmemh(m, 1, 4, "11 22", "@6 33", "44");
    $display("a %h %h %h %h %h %h", m[0], m[1], m[2], m[3], m[4], m[5]);
    m = '{default: 8'h5a};
    $sreadmemh(m, 0, 3, "11 1g", "22");
    $display("b %h %h %h %h", m[0], m[1], m[2], m[3]);
    $finish;
  end
endmodule
