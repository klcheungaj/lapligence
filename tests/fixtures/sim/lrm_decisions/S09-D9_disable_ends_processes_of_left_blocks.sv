// S09-D9: disable ends the processes started in any activation of its target.
// 9.6.2: "All activities enabled within the named block or task shall be
// terminated as well", against its example 3: "If the block is not currently
// executing, the disable has no effect".
// Decision: disable of a named block, task or named fork ends every process
// started in an activation of it, also in one that was left, with the
// processes these started. Processes started elsewhere are not ended.
`timescale 1ns/1ns
module sub(output int n);
  initial begin : body fork #5 n++; join_none end
endmodule

module tb;
  int n, others, u1_n, u2_n;
  process p;
  sub u1(u1_n);
  sub u2(u2_n);

  task spawn;
    fork #5 n++; join_none
  endtask

  function void spawn_in_function;
    fork #5 n++; join_none
  endfunction

  task repeated(int last);
    fork #(5 + last) n++; join_none
    if (last == 2) disable repeated;
    others++;
  endtask

  task nest;
    begin : inner
      fork #5 n++; join_none
    end
  endtask

  task show(string what);
    #20 $display("%-18s n=%0d others=%0d", what, n, others);
    n = 0;
    others = 0;
  endtask

  initial begin
    #2 disable u1.body;
    #8 $display("%-18s %0d %0d", "by instance", u1_n, u2_n);

    begin : left
      fork #5 n++; join_none
    end
    fork #5 others++; join_none
    #2 disable left;
    show("left block");

    for (int i = 0; i < 3; i++) begin : iteration
      fork #5 n++; join_none
      #1 if (i == 1) disable iteration;
      others++;
    end
    show("earlier iterations");

    repeated(0);
    #1 repeated(1);
    #1 repeated(2);
    show("earlier calls");

    begin : below
      fork
        begin
          fork #5 n++; join_none
        end
      join_none
    end
    #2 disable below;
    show("below a finished");

    begin : called
      spawn_in_function();
    end
    #2 disable called;
    show("through a function");

    fork
      spawn();
      #1 spawn();
      #2 disable spawn;
    join
    show("two processes");

    nest();
    #2 disable nest;
    show("outer task");

    nest();
    #2 disable nest.inner;
    show("inner block");

    begin : outer
      begin : second
        fork #5 n++; join_none
      end
      fork #5 others++; join_none
    end
    #2 disable outer.second;
    show("outer goes on");

    fork : some
      #1 others++;
      #5 n++;
    join_any
    #1 disable some;
    show("fork left");

    begin : waited
      fork #5 n++; join_none
    end
    fork #2 disable waited; join_none
    wait fork;
    $display("wait fork returned at %0t", $time);
    show("wait fork");

    begin : awaited
      fork
        begin
          p = process::self();
          #5 n++;
        end
      join_none
    end
    fork
      begin
        p.await();
        $display("await returned: %s", p.status().name());
      end
    join_none
    #2 disable awaited;
    show("await");

    begin : self_made
      fork
        begin
          #2 disable self_made;
          others++;
        end
        #5 n++;
      join_none
    end
    show("by a child");
    $finish;
  end
endmodule
