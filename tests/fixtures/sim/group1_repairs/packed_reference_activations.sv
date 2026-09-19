module tb;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    pair_t shared;
    pair_t elements [0:1];
    task automatic recursive_update(ref pair_t target, const ref pair_t observer, input int n);
        if (n != 0) begin
            target.lo += 8'd1;
            if (observer.lo !== target.lo) $fatal(1, "ref alias is not immediate");
            recursive_update(target, observer, n - 1);
        end
    endtask
    task automatic delayed_update(ref pair_t target);
        target.hi = 8'ha5;
        #1;
        target.lo = 8'h5a;
    endtask
    initial begin
        shared = 16'h1201;
        recursive_update(shared, shared, 3);
        if (shared !== 16'h1204) $fatal(1, "forwarded recursive reference");
        elements[1] = 16'h0000;
        delayed_update(elements[1]);
        if (elements[1] !== 16'ha55a) $fatal(1, "whole array element packed ref");
        $display("packed references passed");
        $finish(0);
    end
endmodule
