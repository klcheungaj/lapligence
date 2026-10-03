// llg-test-fixture: tests/fixtures/sim/mailboxes/scoped_typedefs.sv
// A parameterized mailbox's element type is the type parameter resolved in
// the specializing scope (IEEE 1800-2009 §15.4, §6.18). Module, package and
// compilation-unit typedefs that share one name keep their own widths.
typedef logic [5:0] w_t;

package p;
    typedef logic [11:0] w_t;
endpackage

module child;
    typedef logic [3:0] w_t;
    mailbox #(w_t) box = new();
    w_t got;

    initial begin
        box.put(4'b1x0z);
        box.get(got);
        $display("child=%b", got);
    end
endmodule

module tb;
    typedef logic [129:0] w_t;
    typedef struct packed {
        logic [3:0] tag;
        w_t body;
    } rec_t;
    typedef enum bit [2:0] {A = 1, B = 5} e_t;
    mailbox #(w_t) wide = new();
    mailbox #($unit::w_t) unit_box = new();
    mailbox #(p::w_t) pkg = new();
    mailbox #(rec_t) recs = new();
    mailbox #(e_t) enums = new();
    w_t wide_got;
    $unit::w_t unit_got;
    p::w_t pkg_got;
    rec_t rec_got;
    e_t enum_got;

    child c();

    initial begin
        #1;
        wide.put({2'b1x, 128'h0123_4567_89ab_cdef_fedc_ba98_7654_3210});
        wide.get(wide_got);
        $display("wide=%b %h", wide_got[129:128], wide_got[127:0]);
        unit_box.put(6'h2a);
        unit_box.get(unit_got);
        $display("unit=%h", unit_got);
        pkg.put(12'hzab);
        pkg.get(pkg_got);
        $display("pkg=%h", pkg_got);
        recs.put('{tag: 4'h9, body: 130'h1});
        recs.get(rec_got);
        $display("rec=%h %h", rec_got.tag, rec_got.body);
        enums.put(B);
        enums.get(enum_got);
        $display("enum=%s %0d", enum_got.name(), enum_got);
        $finish;
    end
endmodule
