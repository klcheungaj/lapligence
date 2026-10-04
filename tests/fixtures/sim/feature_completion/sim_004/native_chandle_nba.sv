// SIM-004: nonblocking and delayed writes to persistent chandles and to
// records with chandle and string leaves. IEEE 1800-2009 6.14 (chandles are
// foreign pointers copied by identity), 10.4.1-10.4.2, 4.9.4 and 11.4.11.
import "DPI-C" function chandle foreign_make(int value);
import "DPI-C" function int foreign_value(chandle handle);
import "DPI-C" function void foreign_release(chandle handle);
import "DPI-C" function int foreign_live();
import "DPI-C" function int foreign_bad();

module tb;
  typedef struct {chandle h; string tag; int n;} rec_t;
  chandle h1, h2, cur, a, b;
  rec_t ra, rb, rc;
  logic sel;

  task automatic issue(input chandle h);
    cur <= h;
  endtask

  initial begin
    h1 = foreign_make(11);
    h2 = foreign_make(22);
    cur = h1;
    cur <= h2;
    $display("1 %0d", foreign_value(cur));
    #1 $display("2 %0d", foreign_value(cur));

    cur <= #2 h1;
    cur <= null;
    #1 $display("3 %0d", cur == null);
    #2 $display("4 %0d", foreign_value(cur));

    issue(h2);
    #1 $display("5 %0d", foreign_value(cur));

    a = h1;
    b = h2;
    a <= b;
    b <= a;
    #1 $display("6 %0d %0d", foreign_value(a), foreign_value(b));

    ra = '{h2, "two", 2};
    rb <= ra;
    ra.h = null;
    ra.tag = "gone";
    #1 $display("7 %0d %s %0d %0d", foreign_value(rb.h), rb.tag, rb.n, ra.h == null);
    rb <= #2 '{h1, "one", 1};
    #1 $display("8 %0d %s", foreign_value(rb.h), rb.tag);
    #2 $display("9 %0d %s %0d", foreign_value(rb.h), rb.tag, rb.n);

    ra = '{h1, "first", 5};
    rc = #1 ra;
    $display("10 %0d %s %0d", foreign_value(rc.h), rc.tag, rc.n);
    ra = '{h2, "second", 6};
    fork
      #1 ra.tag = "changed";
    join_none
    rc = #2 ra;
    $display("11 %0d %s %0d", foreign_value(rc.h), rc.tag, rc.n);

    // 11.4.11: an ambiguous predicate keeps equal chandles, otherwise null.
    sel = 1'bx;
    cur = sel ? h1 : h2;
    $display("12 %0d", cur == null);
    cur = sel ? h2 : h2;
    $display("13 %0d", foreign_value(cur));
    sel = 0;
    cur = sel ? h1 : h2;
    $display("14 %0d", foreign_value(cur));
    sel = 1;
    cur = sel ? h1 : h2;
    $display("15 %0d", foreign_value(cur));

    foreign_release(h1);
    foreign_release(h2);
    $display("16 %0d %0d", foreign_live(), foreign_bad());
    $finish(0);
  end
endmodule
