// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/wait_container_records.sv
// The neighbours of reject_wait_record_property that stay legal: a structure
// property that is all packed members has a packed field, and a structure in
// a queue, dynamic or fixed array property publishes through its container's
// change marker, so each wait resumes (SV 9.4.2).
module tb;
    typedef struct {
        int a;
    } plain_t;
    typedef struct {
        int a;
        string s;
    } rec_t;
    class C;
        plain_t p;
        rec_t q[$];
        rec_t d[];
        rec_t f[2];
    endclass
    C h;
    initial begin
        h = new;
        h.q.push_back(rec_t'{0, ""});
        h.d = new[1];
        fork
            begin
                wait (h.p.a == 1);
                $display("%0t packed", $time);
            end
            begin
                wait (h.q[0].a == 1);
                $display("%0t queue", $time);
            end
            begin
                wait (h.d[0].s == "go");
                $display("%0t dynamic", $time);
            end
            begin
                wait (h.f[1].a == 1);
                $display("%0t fixed", $time);
            end
            begin
                #1 h.p.a = 1;
                #1 h.q[0].a = 1;
                #1 h.d[0].s = "go";
                #1 h.f[1].a = 1;
            end
        join
        $finish(0);
    end
endmodule
