// Every weight expression is evaluated exactly once per execution, in source
// order, including the weights of branches that are not chosen. Weight 2 is
// the only nonzero weight, so the chosen branch is fixed.
module tb;
  int calls;
  string log;
  int counter;

  function automatic int weigh(input int id, input int weight);
    calls++;
    log = {log, $sformatf("%0d", id)};
    return weight;
  endfunction

  function automatic int bump(input int step);
    counter += step;
    return 0;
  endfunction

  initial begin
    repeat (3) begin
      log = "";
      randcase
        weigh(1, 0) : $display("unchosen 1");
        weigh(2, 5) : $display("chosen 2, order=%s", log);
        weigh(3, 0) : $display("unchosen 3");
      endcase
    end
    $display("calls=%0d", calls);
    // Each call adds a distinct power of ten, so counter=111 shows that every
    // weight ran exactly once, whatever the order.
    randcase
      bump(1) : $display("unchosen a");
      bump(10) : $display("unchosen b");
      bump(100) : $display("unchosen c");
      1 : $display("constant branch");
    endcase
    $display("counter=%0d", counter);
    // A weight that reads a variable written by an earlier weight sees the
    // earlier write, so source order is observable.
    begin
      int seed = 1;
      randcase
        (seed = 3) - 3 : $display("unchosen d");
        seed - 2 : $display("order: second weight saw the first write");
      endcase
    end
    $finish;
  end
endmodule
