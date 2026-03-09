using System;
using System.Runtime.InteropServices;

namespace OxVba.TestEventServer
{
    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000002")]
    [InterfaceType(ComInterfaceType.InterfaceIsIDispatch)]
    public interface IOxVbaTestEventServerEvents
    {
        [DispId(1)]
        void OnSimpleEvent();

        [DispId(2)]
        void OnValueChanged(int value);

        [DispId(3)]
        void OnPairChanged(int a, int b);
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000003")]
    public interface IOxVbaTestEventServer
    {
        [DispId(101)]
        void FireSimpleEvent();

        [DispId(102)]
        void FireValueChanged(int value);

        [DispId(103)]
        void FirePairChanged(int a);

        [DispId(104)]
        int Ping();
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000004")]
    [ClassInterface(ClassInterfaceType.None)]
    [ComSourceInterfaces(typeof(IOxVbaTestEventServerEvents))]
    [ProgId("OxVba.TestEventServer")]
    public class TestEventServer : IOxVbaTestEventServer
    {
        public delegate void OnSimpleEventHandler();
        public delegate void OnValueChangedHandler(int value);
        public delegate void OnPairChangedHandler(int a, int b);

        public event OnSimpleEventHandler OnSimpleEvent;
        public event OnValueChangedHandler OnValueChanged;
        public event OnPairChangedHandler OnPairChanged;

        public void FireSimpleEvent()
        {
            OnSimpleEvent?.Invoke();
        }

        public void FireValueChanged(int value)
        {
            OnValueChanged?.Invoke(value);
        }

        public void FirePairChanged(int a)
        {
            OnPairChanged?.Invoke(a, a + 1);
        }

        public int Ping()
        {
            return 42;
        }
    }
}
