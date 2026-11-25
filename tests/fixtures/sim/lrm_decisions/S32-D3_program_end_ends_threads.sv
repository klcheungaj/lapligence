// Decision S32-D3: the threads a program's initial procedures left running
// (fork ... join_none, fork ... join_any) end when the last initial procedure
// of that program ends, also while another program keeps the simulation
// running. They run on while another initial procedure of the program runs,
// and `wait fork` keeps its procedure, and with it them, running.
//
// IEEE 1800-2009 24.3 (SystemVerilog-1800-2009.txt L43228-43229):
//   "When all initial procedures within a program have reached their end,
//   that program shall immediately terminate all descendent threads of
//   initial procedures within that program."
// 9.6.1 (L12344-12347):
//   "Simulation also automatically terminates when all its program blocks
//   finish executing (i.e, they reach the end of their execute block),
//   regardless of the status of any child processes (see 24.7). The wait fork
//   statement allows a program block to wait for the completion of all its
//   concurrent threads before exiting."
//
// 9.3.2 starts the join_none threads as concurrent processes of their parent
// and says nothing of programs; 24.7 adds that $exit ends them too. Industry-
// standard simulators may let them run on until all programs have ended.
program first;
  initial begin
    fork
      begin #5 tb.early = 1; end
      begin #15 tb.late = 1; end
      begin #8 tb.left = 1; #8 tb.left = 2; end
    join_none
    #2;
  end
  initial #10;
endprogram
program waiting;
  initial begin
    fork
      begin #15 tb.waited = 1; end
    join_none
    wait fork;
  end
endprogram
program last;
  initial begin
    #30 $display("early=%0d late=%0d left=%0d waited=%0d", tb.early, tb.late, tb.left, tb.waited);
    $finish(0);
  end
endprogram
module tb;
  int early, late, left, waited;
  first f ();
  waiting w ();
  last l ();
endmodule
