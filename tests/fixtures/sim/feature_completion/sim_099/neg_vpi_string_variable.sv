// Gate item G4: a string variable has no fixed-width image, so llg passes
// only string literals to VPI applications.
module tb;
  string s = "cmds.txt";
  initial $input(s);
endmodule
