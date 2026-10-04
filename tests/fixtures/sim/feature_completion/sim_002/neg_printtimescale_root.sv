// SIM-002: `$root` is not a module instance (SV2009 20.4.1 reports "the
// module passed to it"); llg rejects it rather than invent a time scale.
module tb;
  initial $printtimescale($root);
endmodule
