// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_wait_record_property.sv
// A store to a member of an unpacked structure property publishes no change,
// so a wait on it could never resume: it rejects with its location (SV 9.4.2,
// known issue "Waits on record class properties").
module tb;
    typedef struct {
        int a;
        string s;
    } rec_t;
    class C;
        rec_t r;
    endclass
    C h;
    initial begin
        h = new;
        fork
            wait (h.r.a == 1);
            #1 h.r.a = 1;
        join
        $finish;
    end
endmodule
