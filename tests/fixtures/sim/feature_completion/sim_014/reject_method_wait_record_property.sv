// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_method_wait_record_property.sv
// A method's wait on a member of its own unpacked structure property, reached
// through `this`, rejects like a path through a handle (SV 9.4.2).
module tb;
    typedef struct {
        int a;
        string s;
    } rec_t;
    class C;
        rec_t r;
        task t();
            wait (r.s == "go");
            $display("woke");
        endtask
    endclass
    C h;
    initial begin
        h = new;
        fork
            h.t();
            #1 h.r.s = "go";
        join
        $finish;
    end
endmodule
