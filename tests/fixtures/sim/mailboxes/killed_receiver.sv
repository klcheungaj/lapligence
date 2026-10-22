// llg-test-fixture: tests/fixtures/sim/mailboxes/killed_receiver.sv
// IEEE 1800-2009 §15.4.5, §9.4.2 and decision AB-O3: the put that wakes a
// blocked get writes a class handle, a member of an unpacked structure, a
// class property and a whole variable at once, so receivers killed right
// after the put keep them; the handle's change is an event even though its
// receiver never runs again. A whole unpacked-structure and an array-element
// destination are written when the receiver resumes, so a killed receiver
// leaves them unchanged. No message returns to its mailbox.
module tb;
  class C; int x; endclass
  typedef struct { int a; int b; } S;
  mailbox #(C) mh = new();
  mailbox #(int) mm = new();
  mailbox #(int) mp = new();
  mailbox #(S) ms = new();
  mailbox #(int) mi = new();
  mailbox #(int) me = new();
  C got_h, obj, c1;
  S st, sv, tmp;
  int whole;
  int arr[3];
  int handle_events;
  process ph, pm, pp, ps, pi, pe;
  initial begin
    obj = new; c1 = new; c1.x = 7;
    fork
      begin ph = process::self(); mh.get(got_h); $display("h receiver resumed"); end
      begin pm = process::self(); mm.get(st.a); $display("m receiver resumed"); end
      begin pp = process::self(); mp.get(obj.x); $display("p receiver resumed"); end
      begin ps = process::self(); ms.get(sv); $display("s receiver resumed"); end
      begin pi = process::self(); mi.get(whole); $display("i receiver resumed"); end
      begin pe = process::self(); me.get(arr[1]); $display("e receiver resumed"); end
      begin @(got_h); handle_events++; end
    join_none
    #1;
    mh.put(c1); mm.put(11); mp.put(12);
    tmp.a = 13; tmp.b = 14; ms.put(tmp);
    mi.put(15); me.put(16);
    ph.kill(); pm.kill(); pp.kill(); ps.kill(); pi.kill(); pe.kill();
    #1 $display("killed: h=%0d st.a=%0d obj.x=%0d sv.a=%0d whole=%0d arr1=%0d",
                got_h == c1, st.a, obj.x, sv.a, whole, arr[1]);
    $display("handle events=%0d left: %0d %0d %0d %0d %0d %0d", handle_events,
             mh.num(), mm.num(), mp.num(), ms.num(), mi.num(), me.num());
    $finish;
  end
endmodule
