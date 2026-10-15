// llg-test-fixture: tests/fixtures/sim/fork_lifecycle/method_receivers.sv
// IEEE 1800-2009 9.3.2 and 8.11: a `fork ... join_none` branch started in a
// class constructor or function outlives the activation, and its implicit or
// explicit `this` still names the object that started it. The branches here
// call a function and a task method, read and write a property, fork again
// and share an automatic with the function next to the receiver. Object `a`
// is dropped while its branches are pending.
`timescale 1ns / 1ns
class Counter;
    int id;
    int n;

    function void bump(int k);
        n += k;
        $display("%0d id=%0d bump %0d n=%0d", $time, id, k, n);
    endfunction

    task automatic later(int d);
        #d bump(d);
    endtask

    function new(int ident);
        id = ident;
        fork
            begin
                #1 bump(1);
            end
            later(2);
            begin
                #3 n += 100;
                $display("%0d id=%0d prop n=%0d", $time, this.id, n);
            end
            begin
                #4;
                fork
                    begin
                        #1 this.bump(5);
                    end
                join_none
            end
        join_none
    endfunction

    function void spawn(int k);
        int local_k;
        local_k = k;
        fork
            begin
                #6 bump(local_k);
            end
        join_none
        local_k += 1;
    endfunction
endclass

module tb;
    Counter a;
    Counter b;

    initial begin
        a = new(1);
        a.spawn(10);
        a = null;
        #10 b = new(2);
        #10 $display("%0d b.n=%0d", $time, b.n);
        $finish;
    end
endmodule
