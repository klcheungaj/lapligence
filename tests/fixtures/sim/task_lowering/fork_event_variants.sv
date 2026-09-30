module tb;
  event all_go, any_go, none_go, spec_a_go, spec_b_go, rebind_go, spare;
  logic gate_a = 0, gate_b = 0;
  integer hits = 0, triggered = 0, forwarded = 0, nested = 0;
  integer any_return = -1, none_return = -1, specialized = 0;
  integer rebound = 0;

  task automatic forward(input event e, input integer weight);
    @(e);
    forwarded += weight;
  endtask
  task automatic all_waiter(input event e, input integer weight);
    automatic integer local_weight = weight;
    fork
      begin @(e); hits += local_weight; end
      begin wait(e.triggered); triggered += weight; end
      begin forward(e, weight); end
      begin
        fork
          begin @(e); nested += weight; end
          begin #1; end
        join
      end
      begin #2; -> e; end
    join
  endtask
  task automatic any_waiter(input event e, input integer weight);
    fork
      begin @(e); hits += weight; end
      begin wait(e.triggered); triggered += weight; end
      begin forward(e, weight); end
      begin
        fork
          begin @(e); nested += weight; end
          begin #1; end
        join_any
      end
      begin #2; -> e; end
      begin #1; end
    join_any
  endtask
  task automatic none_waiter(input event e, input integer weight);
    fork
      begin @(e); hits += weight; end
      begin wait(e.triggered); triggered += weight; end
      begin forward(e, weight); end
      begin
        fork
          begin @(e); nested += weight; end
          begin #1; end
        join_none
        #1;
      end
      begin #2; -> e; end
    join_none
  endtask
  task automatic specialized_waiter(ref logic gate, input event e, input integer weight);
    @(posedge gate);
    fork
      begin all_waiter(e, weight); specialized++; end
      begin
        fork
          begin @(e); nested += weight; end
          begin #1; end
        join
      end
    join
  endtask
  task automatic joined_rebind(input event e);
    fork
      begin #1; e = spare; #2; -> e; end
      begin #2; @(e); rebound++; end
    join
  endtask
  initial all_waiter(all_go, 1);
  initial begin
    any_waiter(any_go, 10);
    any_return = $time;
    any_go = spare;
    #4;
  end
  initial begin
    none_waiter(none_go, 100);
    none_return = $time;
    none_go = spare;
    #4;
  end
  initial specialized_waiter(gate_a, spec_a_go, 1000);
  initial specialized_waiter(gate_b, spec_b_go, 10000);
  initial joined_rebind(rebind_go);
  initial begin
    #1; gate_a = 1; gate_b = 1;
    #4;
    $display("hits=%0d triggered=%0d forwarded=%0d nested=%0d returns=%0d/%0d specialized=%0d rebound=%0d",
      hits, triggered, forwarded, nested, any_return, none_return, specialized, rebound);
    $finish(0);
  end
endmodule
