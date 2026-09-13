// llg-test-fixture: tests/fixtures/sim/virtual_interfaces/rebind_class.sv
// IEEE 1800-2009 §§25.5, 25.7, 25.9, 25.10: virtual-interface handles keep
// dynamic instance identity across rebinding, class methods, modport imports,
// clocking samples, and fixed-array storage.
interface bus_if #(parameter int W = 4) (input logic clk);
    logic [W-1:0] data = '0;

    task set(input logic [W-1:0] value);
        data = value;
    endtask

    function logic [W-1:0] get();
        return data;
    endfunction

    clocking cb @(posedge clk);
        input #1step data;
    endclocking

    modport master(input clk, output data, import set, get);
endinterface

class Driver;
    virtual bus_if handle;

    function new(virtual bus_if initial_handle);
        handle = initial_handle;
    endfunction

    function void rebind(virtual bus_if next_handle);
        handle = next_handle;
    endfunction

    task put(input logic [3:0] value);
        handle.set(value);
    endtask

    function logic [3:0] sample();
        return handle.cb.data;
    endfunction

    function logic [3:0] value();
        return handle.get();
    endfunction
endclass

module tb;
    logic clk = 0;
    bus_if #(4) first(clk);
    bus_if #(4) second(clk);
    virtual bus_if handle;
    virtual bus_if.master master;
    virtual bus_if #(4) handles [0:1];
    Driver driver;

    initial begin
        handle = first;
        driver = new(handle);
        driver.put(4'h3);
        handles[0] = first;
        handles[1] = second;
        #2;
        $display("first=%0d second=%0d sample=%0d value=%0d", first.data,
            second.data, driver.sample(), driver.value());

        handle = second;
        driver.rebind(handle);
        driver.put(4'h5);
        handle.data = 4'h6;
        #2;
        $display("first=%0d second=%0d sample=%0d value=%0d", first.data,
            second.data, driver.sample(), driver.value());

        master = first;
        master.set(4'h7);
        handles[0].set(4'h8);
        #2;
        $display("modport=%0d array=%0d sample=%0d value=%0d", first.data,
            second.data, driver.sample(), driver.value());
        $finish;
    end

    always #1 clk = ~clk;
endmodule
