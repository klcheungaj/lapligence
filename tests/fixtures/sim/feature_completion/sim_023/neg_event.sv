// Decision S23-D5: an event variable is not a release target.
module tb;
  event e;
  initial release e;
endmodule
