// continue is legal only inside a loop (IEEE 1800-2009 18.17.6 allows
// break and return in code blocks).
module tb;
  initial for (int i = 0; i < 2; i++)
    randsequence (main)
      main : { continue; } ;
    endsequence
endmodule
