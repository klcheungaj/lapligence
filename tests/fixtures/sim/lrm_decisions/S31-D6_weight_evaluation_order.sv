// Decision S31-D6: every weight expression is evaluated exactly once per
// execution of the randcase, in source order, including the weights of
// branches that are not chosen.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31347-31349):
//   "Each weight expression is evaluated at most once (implementations can
//   cache identical expressions) in an unspecified order."
//
// The standard allows any order, and allows a repeated expression to be
// evaluated once. llg evaluates all of them, top to bottom, with no caching,
// so the log below is "123" for every execution; only the second weight is
// nonzero, which makes the chosen branch fixed.
module tb;
  string log;
  int calls;

  function automatic int weigh(input int id, input int weight);
    calls++;
    log = {log, $sformatf("%0d", id)};
    return weight;
  endfunction

  initial begin
    repeat (2) begin
      log = "";
      randcase
        weigh(1, 0) : $display("unchosen 1");
        weigh(2, 5) : $display("chosen 2, evaluation order %s", log);
        weigh(3, 0) : $display("unchosen 3");
      endcase
    end
    $display("weight evaluations: %0d", calls);
    $finish;
  end
endmodule
