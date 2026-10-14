// randcase draws from the calling thread's stream (IEEE 1800-2009 18.16:
// "thread stability"), so a thread's selections do not change when another
// thread is added after it and draws random numbers of its own. Compile with
// -D EXTRA_ACTIVITY to add that thread; only its own lines may change.
module tb;
  int seq_a, seq_b, seq_fork;

  initial begin : thread_a
    repeat (40) begin
      randcase
        1 : seq_a = seq_a * 3 + 0;
        1 : seq_a = seq_a * 3 + 1;
        1 : seq_a = seq_a * 3 + 2;
      endcase
      #1;
    end
    $display("a %0d", seq_a);
  end

  initial begin : thread_b
    fork
      repeat (40) begin
        randcase
          2 : seq_fork = seq_fork * 3 + 0;
          1 : seq_fork = seq_fork * 3 + 1;
        endcase
        #1;
      end
    join_none
    repeat (40) begin
      randcase
        1 : seq_b = seq_b * 3 + 0;
        2 : seq_b = seq_b * 3 + 1;
      endcase
      #1;
    end
    #1 $display("b %0d fork %0d", seq_b, seq_fork);
  end

`ifdef EXTRA_ACTIVITY
  initial begin : thread_extra
    repeat (100) begin
      void'($urandom);
      randcase
        1 : void'($urandom_range(9));
        3 : void'($urandom_range(99));
      endcase
      #1;
    end
    $display("extra done");
  end
`endif

  initial begin
    #200 $finish;
  end
endmodule
