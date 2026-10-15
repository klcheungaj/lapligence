// The examples of IEEE 1800-2009 18.17 run with oracles taken from the
// clause: the outcomes it lists, the outputs it prints and the relations its
// value-passing examples imply. No output depends on the random generator.
module tb;
  string outcome;
  bit seen[string];
  bit bad;
  int flag;
  int depth;
  int device;
  int pushes;
  int cnt;
  bit cond;

  task automatic do_push();
    outcome = {outcome, "push "};
  endtask
  task automatic do_pop();
    outcome = {outcome, "pop "};
  endtask

  initial begin
    // 18.17: the four outcomes of the simple example, all reachable.
    for (int seed = 1; seed <= 64; seed++) begin
      process::self().srandom(seed);
      outcome = "";
      randsequence( main )
        main     : first second done ;
        first    : add | dec ;
        second   : pop | push ;
        done     : { outcome = {outcome, "done"}; } ;
        add      : { outcome = {outcome, "add "}; } ;
        dec      : { outcome = {outcome, "dec "}; } ;
        pop      : { outcome = {outcome, "pop "}; } ;
        push     : { outcome = {outcome, "push "}; } ;
      endsequence
      seen[outcome] = 1;
    end
    foreach (seen[key]) $display("18.17 outcome: %s", key);
    seen.delete();

    // 18.17.1: the weighted rewrite keeps the same outcomes.
    for (int seed = 1; seed <= 64; seed++) begin
      process::self().srandom(seed);
      outcome = "";
      randsequence( main )
        main     : first ;
        first    : add := 3
                 | dec := (1 + 1)
                 ;
        add      : { outcome = "add"; } ;
        dec      : { outcome = "dec"; } ;
      endsequence
      seen[outcome] = 1;
    end
    foreach (seen[key]) $display("18.17.1 outcome: %s", key);
    seen.delete();

    // 18.17.2: depth follows the if-else production.
    outcome = "";
    depth = 0;
    repeat (4) begin
      randsequence()
        PP_PO : if ( depth < 2 ) PUSH else POP ;
        PUSH : { ++depth; do_push(); };
        POP   : { --depth; do_pop(); };
      endsequence
    end
    $display("18.17.2: %sdepth=%0d", outcome, depth);

    // 18.17.3: the case production.
    outcome = "";
    for (device = 0; device < 10; device++) begin
      randsequence()
        SELECT : case ( device & 7 )
           0        : NETWORK ;
           1, 2     : DISK ;
           default : MEMORY ;
        endcase ;
        NETWORK : { outcome = {outcome, "N"}; } ;
        DISK : { outcome = {outcome, "D"}; } ;
        MEMORY : { outcome = {outcome, "M"}; } ;
      endsequence
    end
    $display("18.17.3: %s", outcome);

    // 18.17.4: the repeat count comes from $urandom_range(2, 6).
    bad = 1'b0;
    for (int seed = 1; seed <= 64; seed++) begin
      process::self().srandom(seed);
      pushes = 0;
      randsequence()
        PUSH_OPER : repeat( $urandom_range( 2, 6 ) ) PUSH ;
        PUSH      : { pushes++; } ;
      endsequence
      if (pushes < 2 || pushes > 6) bad = 1'b1;
      seen[$sformatf("%0d", pushes)] = 1;
    end
    $display("18.17.4: in range %0d, counts seen %0d", !bad, seen.num());
    seen.delete();

    // 18.17.6: the return example prints the clause's three lines.
    for (flag = 0; flag < 3; flag++) begin
      outcome = "";
      randsequence()
        TOP : P1 P2 ;
        P1 : A B C ;
        P2 : A { if( flag == 1 ) return; } B C ;
        A   : { outcome = {outcome, " A"}; } ;
        B   : { if( flag == 2 ) return; outcome = {outcome, " B"}; } ;
        C   : { outcome = {outcome, " C"}; } ;
      endsequence
      $display("18.17.6: flag == %0d ==>%s", flag, outcome);
    end

    // 18.17.6: break leaves the randsequence block.
    for (int fifo_length = 0; fifo_length < 2; fifo_length++) begin
      outcome = "";
      randsequence()
        WRITE   : SETUP DATA ;
        SETUP : { if( fifo_length >= 1 ) break; } COMMAND ;
        COMMAND : { outcome = {outcome, "command "}; } ;
        DATA   : { outcome = {outcome, "data "}; } ;
      endsequence
      $display("18.17.6: fifo_length %0d: %snext_statement", fifo_length, outcome);
    end

    // 18.17.7: the gen production with a default argument.
    for (int seed = 1; seed <= 32; seed++) begin
      process::self().srandom(seed);
      outcome = "";
      randsequence( main )
        main                                           : first second gen ;
        first                                          : add | dec ;
        second                                         : pop | push ;
        add                                            : gen("add") ;
        dec                                            : gen("dec") ;
        pop                                            : gen("pop") ;
        push                                           : gen("push") ;
        gen( string s = "done" )                       : { outcome = {outcome, s, " "}; } ;
      endsequence
      seen[outcome] = 1;
    end
    foreach (seen[key]) $display("18.17.7 gen: %s", key);
    seen.delete();

    // 18.17.7 example 1: operator weights 5:2:1, values in two slots.
    for (int seed = 1; seed <= 64; seed++) begin
      process::self().srandom(seed);
      randsequence( bin_op )
        void bin_op     : value operator value // void type is optional
                          { outcome = $sformatf("%s %b %b", operator, value[1], value[2]); }
                          ;
        bit [7:0] value : { return 8'b1010_0101; } ;
        string operator :   add := 5 { return "+" ; }
                          | dec := 2 { return "-" ; }
                          | mult := 1 { return "*" ; }
                          ;
        add : { } ;
        dec : { } ;
        mult : { } ;
      endsequence
      seen[outcome] = 1;
    end
    foreach (seen[key]) $display("18.17.7 bin_op: %s", key);
    seen.delete();

    // 18.17.7 example 2: B returns the last of its 1 to 3 C values, and the
    // 5 repeated C values run on from the first B.
    bad = 1'b0;
    for (int seed = 1; seed <= 64; seed++) begin
      process::self().srandom(seed);
      cond = seed[0];
      randsequence( A )
        void A   : A1 A2;
        void A1 : { cnt = 1; } B repeat(5) C B
                 {
                   if (B[1] < 2 || B[1] > 4 || C != B[1] + 5 || B[2] - C < 1 || B[2] - C > 3)
                     bad = 1'b1;
                 }
                 ;
        void A2 : if (cond) D(5) else D(20)
                 { if (cond ? (D[1] != 5 || D[2] != 0) : (D[1] != 0 || D[2] != 20)) bad = 1'b1; }
                 ;
        int B    : C { return C;}
                 | C C { return C[2]; }
                 | C C C { return C[3]; }
                 ;
        int C    : { cnt = cnt + 1; return cnt; };
        int D (int prm) : { return prm; };
      endsequence
    end
    $display("18.17.7 example 2 relations hold: %0d", !bad);
    $finish;
  end
endmodule
