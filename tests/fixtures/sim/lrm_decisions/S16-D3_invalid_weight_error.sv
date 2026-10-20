// Decision S16-D3: a negative rule weight is a run-time error that ends the
// simulation at once: the production generates nothing, nothing after it
// runs and final procedures do not run (llg: an error on stderr, exit
// status 1). X and Z bits of a weight read as 0, as in randcase (S31-D5),
// and are no error. When every weight is zero no rule is generated; that
// only warns.
//
// IEEE 1800-2009 18.17.1 (SystemVerilog-1800-2009.txt L31489-31490):
//   "A weight_specification shall evaluate to an integral non-negative
//   value."
// The clause states the requirement but not what happens when it is broken;
// llg follows the industry-standard behaviour.
module tb;
  int minus = -1;
  logic [3:0] unknown = 4'b000x;
  int na, nb;

  final begin
    $display("final block");
  end

  initial begin
    repeat (20)
      randsequence (main)
        main : zero unknown_weight ;
        zero : a := 0 | b := 0 ;
        unknown_weight : a := unknown | b := 1 ;
        a : { na++; } ;
        b : { nb++; } ;
      endsequence
    $display("unknown weight: a %0d b %0d", na, nb);
    randsequence (main)
      main : first negative { $display("after"); } ;
      first : { $display("first"); } ;
      negative : c := minus | d := 1 ;
      c : { $display("c"); } ;
      d : { $display("d"); } ;
    endsequence
    $display("done");
    $finish;
  end
endmodule
