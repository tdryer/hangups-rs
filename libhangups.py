import datetime
import json

import ctypes

_libhangups = ctypes.CDLL('target/debug/libhangups.so')
_libhangups.libhangups_client_create.restype = ctypes.c_void_p
_libhangups.libhangups_client_create.argtypes = []
_libhangups.libhangups_client_receive.restype = ctypes.POINTER(ctypes.c_char_p)
_libhangups.libhangups_client_receive.argtypes = [ctypes.c_void_p, ctypes.c_uint64]
_libhangups.libhangups_client_destroy.restype = ctypes.c_void_p
_libhangups.libhangups_client_destroy.argtypes = [ctypes.c_void_p]
_libhangups.libhangups_destroy_received.restype = ctypes.c_void_p
_libhangups.libhangups_destroy_received.argtypes = [ctypes.POINTER(ctypes.c_char_p)]


class Client():

    def __init__(self):
        self._client = _libhangups.libhangups_client_create()
        assert self._client

    def __del__(self):
        _libhangups.libhangups_client_destroy(self._client)

    def receive(self, timeout):
        assert isinstance(timeout, datetime.timedelta)
        res_ptr = _libhangups.libhangups_client_receive(
            self._client, int(timeout.total_seconds() * 1000)
        )
        assert res_ptr
        res_str = ctypes.cast(res_ptr, ctypes.c_char_p).value
        _libhangups.libhangups_destroy_received(res_ptr)
        return json.loads(res_str)


def main():
    try:
        client = Client()
        while True:
            state_update = client.receive(datetime.timedelta(seconds=1))
            assert state_update is not None
            if state_update:
                print(json.dumps(state_update, indent=4))
    except KeyboardInterrupt:
        pass


if __name__ == '__main__':
    main()
