// Many processes, nonblocking assignments, zero-delay gates, #0 loops, event
// triggers, mailboxes and semaphores at one time. Every line has a set of
// accepted values that readme.md derives; the test checks membership.
`timescale 1ns/1ns

module gate_chain #(parameter int N = 16) (input wire a, output wire y);
  wire [N:0] n;
  assign n[0] = a;
  for (genvar i = 0; i < N; i++) begin : g
    if (i % 2) buf #0 b(n[i + 1], n[i]);
    else not #0 b(n[i + 1], n[i]);
  end
  assign y = n[N];
endmodule

module tb;
  parameter int N = 24;

  // Blocking writes of one variable.
  int race, ran[N];
  for (genvar i = 0; i < N; i++) begin : g_race
    initial begin
      #1 race = i;
      ran[i]++;
    end
  end
  initial #2 $display("race %0d ran %0d", race, ran.sum());

  // Nonblocking writes of one variable by many processes, two by each.
  int nb, nb2, nchanges;
  bit nclk;
  initial begin
    nb = -1;
    nb2 = -1;
  end
  for (genvar i = 0; i < N; i++) begin : g_nb
    always @(posedge nclk) begin
      nb <= i;
      nb2 <= 100 + i;
      nb2 <= 1000 + i;
    end
  end
  always @(nb) if (nclk) nchanges++;
  initial begin
    #3 nclk = 1;
    #1 $display("nonblocking %0d %0d changes %0d", nb, nb2, nchanges);
  end

  // Triggers and waits of one time step.
  event ev, nev;
  int early[N], late[N], persisted[N], after_nb[N];
  for (genvar i = 0; i < N; i++) begin : g_ev
    initial begin
      #4;
      @ev early[i]++;
    end
    initial begin
      #6;
      @ev late[i]++;
    end
    initial begin
      #6;
      wait (ev.triggered) persisted[i]++;
    end
    initial begin
      #6;
      @nev after_nb[i]++;
    end
  end
  initial #6 ->ev;
  initial begin
    #6 ->>nev;
    #1 $display("events %0d %0d %0d %0d", early.sum(), late.sum(), persisted.sum(), after_nb.sum());
  end

  // Zero-delay gates whose inputs switch at one time.
  reg [7:0] ga = 0;
  wire [7:0] gy;
  int gchanges;
  for (genvar i = 0; i < 8; i++) begin : g_chain
    gate_chain c(ga[i], gy[i]);
  end
  wire parity = ^gy;
  always @(parity) gchanges++;
  initial begin
    #8 gchanges = 0;
    ga = 8'hff;
    ga = 8'h7f;
    #1 $display("gates %h parity %b changes %0d", gy, parity, gchanges);
  end

  // #0 loops.
  int turns[N], ended[N], sample;
  for (genvar i = 0; i < N; i++) begin : g_zero
    initial begin
      #10;
      repeat (5) begin
        turns[i]++;
        #0;
      end
      ended[i] = 1;
    end
  end
  initial begin
    #10;
    #0 sample = turns.sum();
    #1 $display("zero %0d %0d sample %0d", turns.sum(), ended.sum(), sample);
  end

  // Producers, consumers and pollers of one bounded mailbox.
  mailbox #(int) mb = new(2);
  int got[$], taken[N], polled[N];
  for (genvar i = 0; i < N; i++) begin : g_mailbox
    initial begin
      #12;
      for (int k = 1; k <= 3; k++) mb.put(i * 10 + k);
    end
    initial begin
      int v;
      #12 mb.get(v);
      got.push_back(v);
      taken[i]++;
    end
    initial begin
      int v;
      #12;
      repeat (4) begin
        if (mb.try_get(v)) begin
          got.push_back(v);
          polled[i]++;
        end
        #0;
      end
    end
  end
  initial begin
    int seen[int];
    #13;
    foreach (got[i]) seen[got[i]]++;
    $display("mailbox got %0d distinct %0d left %0d taken %0d polled %0d", got.size(), seen.num(),
             mb.num(), taken.sum(), polled.sum());
  end

  // Keys of one semaphore.
  semaphore sem = new(3);
  int held[N], peak, entered[N];
  for (genvar i = 0; i < N; i++) begin : g_semaphore
    initial begin
      #15 sem.get(1 + i % 2);
      held[i] = 1 + i % 2;
      if (held.sum() > peak) peak = held.sum();
      #0 held[i] = 0;
      entered[i]++;
      sem.put(1 + i % 2);
    end
  end
  initial #16 $display("semaphore peak %0d entered %0d all back %0d", peak, entered.sum(), sem.try_get(3));

  // Children that end in the time step in which they are disabled.
  int done[N], finished, killed;
  initial begin
    process p[N];
    #17;
    for (int i = 0; i < N; i++) begin
      automatic int k = i;
      fork
        begin
          p[k] = process::self();
          #1 done[k] = 1;
        end
      join_none
    end
    #1 disable fork;
    #0;
    foreach (p[i]) begin
      if (p[i].status() == process::FINISHED) finished++;
      if (p[i].status() == process::KILLED) killed++;
    end
    $display("disable done %0d finished %0d killed %0d", done.sum(), finished, killed);
  end

  // The first of many children that end in one time step.
  int any[N], at_join;
  initial begin
    #19;
    fork
      begin
        for (int i = 0; i < N; i++) begin
          automatic int k = i;
          fork
            #1 any[k] = 1;
          join_none
        end
        wait fork;
      end
      #1 at_join = any.sum();
    join_any
    #1 $display("join any %0d all %0d", at_join, any.sum());
  end

  // Strobes and monitors of a step see its last values.
  int s;
  for (genvar i = 0; i < N; i++) begin : g_strobe
    initial begin
      #22 s = i;
      #0 s = 100 + i;
    end
    initial #22 s <= 1000;
  end
  initial #22 $strobe("strobe %0d", s);

  initial #30 $finish;
endmodule
