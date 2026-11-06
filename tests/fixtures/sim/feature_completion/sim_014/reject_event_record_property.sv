// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_event_record_property.sv
// The event control form of reject_wait_record_property, one handle further.
module tb;
    typedef struct {
        int a;
        string s;
    } rec_t;
    class C;
        rec_t r;
    endclass
    class D;
        C c;
    endclass
    D d;
    initial begin
        d = new;
        d.c = new;
        fork
            @(posedge d.c.r.a) $display("woke");
            #1 d.c.r.a = 1;
        join
        $finish;
    end
endmodule
