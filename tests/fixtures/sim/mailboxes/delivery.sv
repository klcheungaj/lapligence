// llg-test-fixture: tests/fixtures/sim/mailboxes/delivery.sv
// IEEE 1800-2009 §15.4.5 and decision AB-O3: the put that wakes a blocked
// get completes it. A whole packed, real or string variable, a `ref` formal,
// an automatic variable of the waiting activation, a class property and a
// class handle are written at the put; an array element is written when the
// receiver resumes, with the element selector fixed at the call.
module tb;
    timeunit 1ns;
    timeprecision 1ns;

    class C;
        int x;
    endclass

    mailbox #(int) mi = new();
    mailbox #(real) mr = new();
    mailbox #(string) ms = new();
    mailbox #(int) me = new();
    mailbox #(int) mp = new();
    mailbox #(int) mf = new();
    mailbox #(int) ml = new();
    mailbox #(C) mh = new();
    int whole;
    real r;
    string s;
    int arr[3];
    int i;
    int via_ref;
    C h, h1, h2, got_h;

    task automatic take_ref(ref int dest);
        mf.get(dest);
    endtask

    task automatic local_view();
        int v;
        v = -1;
        fork
            ml.get(v);
        join_none
        #1 ml.put(8);
        $display("local at put v=%0d", v);
    endtask

    initial begin
        h1 = new;
        h2 = new;
        h = h1;
        i = 0;
        fork
            mi.get(whole);
            mr.get(r);
            ms.get(s);
            me.get(arr[i]);
            mp.get(h.x);
            take_ref(via_ref);
            mh.get(got_h);
        join_none
        #1;
        i = 2;
        h = h2;
        mi.put(1);
        mr.put(2.5);
        ms.put("three");
        me.put(4);
        mp.put(5);
        mf.put(6);
        mh.put(h1);
        $display("at put whole=%0d r=%0.1f s=%s ref=%0d arr=%0d,%0d,%0d h1.x=%0d got_h=%0d",
            whole, r, s, via_ref, arr[0], arr[1], arr[2], h1.x, got_h != null);
        #0;
        $display("after resume arr=%0d,%0d,%0d h1.x=%0d h2.x=%0d got_h=%0d",
            arr[0], arr[1], arr[2], h1.x, h2.x, got_h == h1);
        local_view();
        $finish;
    end
endmodule
