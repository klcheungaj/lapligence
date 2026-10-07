// SIM-015: suspend, resume and kill behave the same whether the target waits
// on a delay, a named event, `wait fork`, a semaphore, a mailbox or another
// process's completion (SV 9.7, 9.6.1, 15.3, 15.4).
module tb;
  process t_h, e_h, f_h, s_h, m_h, a_h, k_h;
  event ev;
  semaphore sem, sem_k;
  mailbox #(int) mbx;
  int got;

  function automatic string st(process p);
    return p.status().name();
  endfunction

  task automatic all_states(input string label);
    $display("%s: %s %s %s %s %s %s %s", label, st(t_h), st(e_h), st(f_h), st(s_h),
             st(m_h), st(a_h), st(k_h));
  endtask

  initial begin
    sem = new(0);
    sem_k = new(0);
    mbx = new();
    fork
      begin t_h = process::self(); #10; $display("t done %0d", $time); end
      begin e_h = process::self(); @ev; $display("e done %0d", $time); end
      begin
        f_h = process::self();
        fork #8; join_none
        wait fork;
        $display("f done %0d", $time);
      end
      begin s_h = process::self(); sem.get(1); $display("s done %0d", $time); end
      begin m_h = process::self(); mbx.get(got); $display("m done %0d got %0d", $time, got); end
      begin a_h = process::self(); #0; t_h.await(); $display("a done %0d", $time); end
      begin k_h = process::self(); sem_k.get(1); $display("FAIL killed waiter resumed"); end
    join_none
    #1;
    all_states("1");
    t_h.suspend(); e_h.suspend(); f_h.suspend(); s_h.suspend();
    m_h.suspend(); a_h.suspend(); k_h.suspend();
    all_states("1s");
    #1;
    sem.put(1);
    mbx.put(42);
    k_h.kill();
    sem_k.put(1);
    $display("2: %s %s %s %0d", st(s_h), st(m_h), st(k_h), sem_k.try_get(1));
    #1;
    s_h.resume();
    #1;
    m_h.resume();
    #1;
    t_h.resume();
    e_h.resume();
    $display("5: %s %s %s %s", st(t_h), st(e_h), st(s_h), st(m_h));
    #1;
    ->ev;
    #3;
    $display("9: %s %s", st(f_h), st(e_h));
    f_h.resume();
    #2;
    $display("11: %s %s", st(t_h), st(a_h));
    a_h.resume();
    a_h.await();
    all_states("end");
    $finish;
  end
endmodule
