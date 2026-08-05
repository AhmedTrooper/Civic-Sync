import { describe, it, expect, beforeEach } from 'vitest';
import { useAdminStore } from '../src/store/adminStore';

describe('Admin Store', () => {
	beforeEach(() => {
		// Reset the store before each test if necessary
		useAdminStore.setState({
			centers: [],
			incidents: [],
			resources: [],
			isLoading: false,
			error: null,
		});
	});

	it('should start with initial state', () => {
		const state = useAdminStore.getState();
		expect(state.centers).toEqual([]);
		expect(state.incidents).toEqual([]);
		expect(state.resources).toEqual([]);
		expect(state.isLoading).toBe(false);
		expect(state.error).toBeNull();
	});

	it('should allow setting centers', () => {
		const dummyCenters = [
			{
				id: '123',
				name: 'Test Center',
				is_core_center: true,
				latitude: 0,
				longitude: 0,
				created_at: new Date().toISOString(),
				updated_at: new Date().toISOString(),
				server_synced_at: null,
			},
		];

		useAdminStore.setState({ centers: dummyCenters });
		expect(useAdminStore.getState().centers).toEqual(dummyCenters);
	});
});
