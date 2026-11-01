// Decision S3-D5: when several inputs of a sequential UDP change before the
// UDP is evaluated, each change is one transition, taken in the order the
// inputs changed (the statement order of the writer, the order of
// nonblocking updates, or the order in which processes wrote them). Each
// transition sees the new values of the inputs that changed before it and
// the old values of those that change after it. Inputs whose changes the
// gate cannot tell apart keep port order (decision S3-D2).
//
// IEEE 1800-2009 4.3 (SystemVerilog-1800-2009.txt L3038-3043): "Every change
// in state of a net or variable in the system description being simulated
// is considered an update event. Processes are sensitive to update events.
// When an update event is executed, all the processes that are sensitive to
// that event are considered for evaluation in an arbitrary order." 4.7
// L3465-3471: "statements without time-control constructs in procedural
// blocks do not have to be executed as one event ... At any time while
// evaluating a procedural statement, the simulator may suspend execution and
// place the partially completed event as a pending event in the event
// region." 29.5 L48956 (29.6): "In level-sensitive behavior, the values of
// the inputs and the current state are sufficient to determine the output
// value." Taking the changes in their order gives the result of evaluating
// the UDP after each statement, which 4.7 permits; port order could give a
// state that no interleaving produces (a latch closed first and then fed new
// data would follow that data).
primitive latch (q, d, en);
    output q;
    reg q;
    input d, en;
    table
    // d en : q : q+
       0  1 : ? : 0 ;
       1  1 : ? : 1 ;
       ?  0 : ? : - ;
    endtable
endprimitive

primitive dff (q, clk, d);
    output q;
    reg q;
    input clk, d;
    initial q = 0;
    table
    // clk d : q : q+
       r    0 : ? : 0 ;
       r    1 : ? : 1 ;
       (?0) ? : ? : - ;
       ?    * : ? : - ;
    endtable
endprimitive

// Rising b sets the state and rising c clears it: the state tells which of
// the two changed last.
primitive last_bc (q, a, b, c);
    output q;
    reg q;
    input a, b, c;
    table
    // a b c : q : q+
       * ? ? : ? : - ;
       ? r ? : ? : 1 ;
       ? n ? : ? : - ;
       ? (0x) ? : ? : - ;
       ? (x1) ? : ? : - ;
       ? ? r : ? : 0 ;
       ? ? n : ? : - ;
       ? ? (0x) : ? : - ;
       ? ? (x1) : ? : - ;
    endtable
endprimitive

module tb;
    reg d1, e1, d2, e2, d3, e3, d4, e4;
    wire q1, q2, q3, q4;
    latch l1 (q1, d1, e1);
    latch l2 (q2, d2, e2);
    latch l3 (q3, d3, e3);
    latch l4 (q4, d4, e4);
    reg c1, x1, c2, x2;
    wire f1, f2;
    dff g1 (f1, c1, x1);
    dff g2 (f2, c2, x2);
    reg a1, b1, k1, a2, b2, k2;
    wire o1, o2;
    last_bc u1 (o1, a1, b1, k1);
    last_bc u2 (o2, a2, b2, k2);
    reg [3:0] v;
    wire w;
    latch l5 (w, v[2], v[0]);
    initial begin
        {d1, e1, d2, e2, d3, e3, d4, e4} = 8'hff;
        {c1, x1, c2, x2} = 4'h0;
        {a1, b1, k1, a2, b2, k2} = 6'h00;
        v = 4'b0101;
        #1 $display("open q1=%b q2=%b q3=%b q4=%b w=%b", q1, q2, q3, q4, w);
        e1 = 1'b0;
        d1 = 1'b0;
        d2 = 1'b0;
        e2 = 1'b0;
        v[0] = 1'b0;
        v[2] = 1'b0;
        c1 = 1'b1;
        x1 = 1'b1;
        x2 = 1'b1;
        c2 = 1'b1;
        a1 = 1'b1;
        k1 = 1'b1;
        b1 = 1'b1;
        b2 = 1'b1;
        a2 = 1'b1;
        k2 = 1'b1;
        #1 $display("closed_first q1=%b data_first q2=%b bits w=%b", q1, q2, w);
        $display("clock_first f1=%b data_first f2=%b", f1, f2);
        $display("b_last o1=%b c_last o2=%b", o1, o2);
        $display("nonblocking q3=%b q4=%b", q3, q4);
        $finish;
    end
    initial begin
        #1 e3 <= 1'b0;
        d3 <= 1'b0;
        d4 <= 1'b0;
        e4 <= 1'b0;
    end
endmodule
