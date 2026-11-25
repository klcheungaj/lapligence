// Gate item G4: a string variable has no fixed-width image, so llg passes
// only string literals and string parameters to VPI applications.
module tb;
  string s = "cmds.txt";
  initial $input(s);
endmodule
